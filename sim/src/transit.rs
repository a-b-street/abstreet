use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Time;
use map_model::{Map, Path, TransitRoute, TransitRouteID, TransitStopID};

use crate::sim::Ctx;
use crate::{
    AgentID, CarID, DrivingSimState, Event, PedestrianID, PersonID, Router, TripID, TripManager,
    TripPhaseType, UnzoomedAgent, VehicleType, WalkingSimState,
};

// These index stops along a route, not stops along a single sidewalk.
type StopIdx = usize;

#[derive(Serialize, Deserialize, Clone)]
struct Route {
    // Entry i is the path to drive to stop i. The very last path is to drive from the last step to
    // the place where the vehicle vanishes.
    paths: Vec<Path>,
    stops: Vec<TransitStopID>,
    active_vehicles: BTreeSet<CarID>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Bus {
    car: CarID,
    route: TransitRouteID,
    /// Where does each passenger want to deboard?
    passengers: Vec<(PersonID, Option<TransitStopID>)>,
    state: BusState,
}

#[derive(Serialize, Deserialize, Clone)]
enum BusState {
    DrivingToStop(StopIdx),
    AtStop(StopIdx),
    // Or to the end of the lane with the last stop
    DrivingOffMap,
    Finished,
}

/// Manages public transit vehicles (buses and trains) that follow a route. The transit model is
/// currently kind of broken, so not describing the state machine yet.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct TransitSimState {
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    buses: BTreeMap<CarID, Bus>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    routes: BTreeMap<TransitRouteID, Route>,
    /// waiting at => (ped, route, bound for, started waiting)
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    peds_waiting:
        BTreeMap<TransitStopID, Vec<(PedestrianID, TransitRouteID, Option<TransitStopID>, Time)>>,

    events: Vec<Event>,
}

impl TransitSimState {
    pub fn new(map: &Map) -> TransitSimState {
        // Keep this filled out always so get_passengers can return &Vec without a hassle
        let mut peds_waiting = BTreeMap::new();
        for ts in map.all_transit_stops().keys() {
            peds_waiting.insert(*ts, Vec::new());
        }

        TransitSimState {
            buses: BTreeMap::new(),
            routes: BTreeMap::new(),
            peds_waiting,
            events: Vec::new(),
        }
    }

    /// Returns the path for the first leg.
    pub fn create_empty_route(&mut self, bus_route: &TransitRoute, map: &Map) -> Path {
        self.routes
            .entry(bus_route.id)
            .or_insert_with(|| match bus_route.all_paths(map) {
                Ok(paths) => {
                    let stops = bus_route.stops.clone();
                    assert_eq!(paths.len(), stops.len() + 1);
                    Route {
                        stops,
                        paths,
                        active_vehicles: BTreeSet::new(),
                    }
                }
                Err(err) => {
                    panic!("{} wound up with bad paths: {}", bus_route.short_name, err);
                }
            });

        self.routes[&bus_route.id].paths[0].clone()
    }

    pub fn bus_created(&mut self, bus: CarID, r: TransitRouteID) {
        let route = self.routes.get_mut(&r).unwrap();
        route.active_vehicles.insert(bus);
        self.buses.insert(
            bus,
            Bus {
                car: bus,
                route: r,
                passengers: Vec::new(),
                state: BusState::DrivingToStop(0),
            },
        );
    }

    /// If true, the bus is idling. If false, the bus actually arrived at a border and should now
    /// vanish.
    ///
    /// TODO Misnomer -- callback from Router::follow_bus_route
    pub fn bus_arrived_at_stop(
        &mut self,
        now: Time,
        id: CarID,
        trips: &mut TripManager,
        walking: &mut WalkingSimState,
        ctx: &mut Ctx,
    ) -> bool {
        let mut bus = self.buses.get_mut(&id).unwrap();
        match bus.state {
            BusState::DrivingToStop(stop_idx) => {
                bus.state = BusState::AtStop(stop_idx);
                let stop1 = self.routes[&bus.route].stops[stop_idx];
                self.events
                    .push(Event::BusArrivedAtStop(id, bus.route, stop1));

                // Deboard existing passengers.
                let mut still_riding = Vec::new();
                for (person, maybe_stop2) in bus.passengers.drain(..) {
                    if Some(stop1) == maybe_stop2 {
                        trips.person_left_bus(now, person, bus.car, ctx);
                        self.events.push(Event::PassengerAlightsTransit(
                            person, bus.car, bus.route, stop1,
                        ));
                    } else {
                        still_riding.push((person, maybe_stop2));
                    }
                }
                bus.passengers = still_riding;

                // Board new passengers.
                let mut still_waiting = Vec::new();
                for (ped, route, maybe_stop2, started_waiting) in
                    self.peds_waiting.remove(&stop1).unwrap()
                {
                    if bus.route == route {
                        let (trip, person) = trips.ped_boarded_bus(
                            now,
                            ped,
                            bus.car,
                            now - started_waiting,
                            walking,
                        );
                        self.events.push(Event::PassengerBoardsTransit(
                            person,
                            bus.car,
                            bus.route,
                            stop1,
                            now - started_waiting,
                        ));
                        // TODO Recording the PathRequest for the passenger is actually hard. We
                        // don't want to route directly between their first and last stop, because
                        // there might be a much shorter path there. Should we record a leg per leg
                        // of the transit route being followed?
                        self.events.push(Event::TripPhaseStarting(
                            trip,
                            person,
                            None,
                            TripPhaseType::RidingBus(route, stop1, bus.car),
                        ));
                        bus.passengers.push((person, maybe_stop2));
                    } else {
                        still_waiting.push((ped, route, maybe_stop2, started_waiting));
                    }
                }
                self.peds_waiting.insert(stop1, still_waiting);
                true
            }
            BusState::DrivingOffMap => {
                self.routes
                    .get_mut(&bus.route)
                    .unwrap()
                    .active_vehicles
                    .remove(&id);
                for (person, maybe_stop2) in bus.passengers.drain(..) {
                    if let Some(stop2) = maybe_stop2 {
                        panic!(
                            "{} fell asleep on {} and just rode off-map, but they were supposed \
                             to hop off at {}",
                            person, bus.car, stop2
                        );
                    }
                    trips.transit_rider_reached_border(now, person, id, ctx);
                }
                bus.state = BusState::Finished;
                false
            }
            BusState::AtStop(_) | BusState::Finished => unreachable!(),
        }
    }

    pub fn bus_departed_from_stop(&mut self, id: CarID, _: &Map) -> Router {
        let mut bus = self.buses.get_mut(&id).unwrap();
        let route = self.routes.get_mut(&bus.route).unwrap();
        match bus.state {
            BusState::AtStop(stop_idx) => {
                self.events.push(Event::BusDepartedFromStop(
                    id,
                    bus.route,
                    route.stops[stop_idx],
                ));

                if stop_idx == route.stops.len() - 1 {
                    bus.state = BusState::DrivingOffMap;
                } else {
                    bus.state = BusState::DrivingToStop(stop_idx + 1);
                }
                Router::follow_bus_route(id, route.paths[stop_idx + 1].clone())
            }
            BusState::DrivingToStop(_) | BusState::DrivingOffMap | BusState::Finished => {
                unreachable!()
            }
        }
    }

    /// Returns the bus if the pedestrian boarded immediately.
    pub fn ped_waiting_for_bus(
        &mut self,
        now: Time,
        ped: PedestrianID,
        trip: TripID,
        person: PersonID,
        stop1: TransitStopID,
        route_id: TransitRouteID,
        maybe_stop2: Option<TransitStopID>,
        _: &Map,
    ) -> Option<CarID> {
        assert!(Some(stop1) != maybe_stop2);
        if let Some(route) = self.routes.get(&route_id) {
            for bus in &route.active_vehicles {
                if let BusState::AtStop(idx) = self.buses[bus].state {
                    if route.stops[idx] == stop1 {
                        self.buses
                            .get_mut(bus)
                            .unwrap()
                            .passengers
                            .push((person, maybe_stop2));
                        // TODO Same problem as elsewhere with recording the PathRequest
                        self.events.push(Event::TripPhaseStarting(
                            trip,
                            person,
                            None,
                            TripPhaseType::RidingBus(route_id, stop1, *bus),
                        ));
                        return Some(*bus);
                    }
                }
            }
        } else {
            println!(
                "WARNING: {} waiting for {}, but that route hasn't been instantiated",
                ped, route_id
            );
        }

        self.peds_waiting
            .get_mut(&stop1)
            .unwrap()
            .push((ped, route_id, maybe_stop2, now));
        None
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    pub fn get_passengers(&self, bus: CarID) -> &Vec<(PersonID, Option<TransitStopID>)> {
        &self.buses[&bus].passengers
    }

    pub fn bus_route(&self, bus: CarID) -> TransitRouteID {
        self.buses[&bus].route
    }

    /// also stop idx that the bus is coming from
    pub fn buses_for_route(&self, route: TransitRouteID) -> Vec<(CarID, Option<usize>)> {
        if let Some(r) = self.routes.get(&route) {
            r.active_vehicles
                .iter()
                .map(|bus| {
                    let stop = match self.buses[bus].state {
                        BusState::DrivingToStop(idx) => {
                            if idx == 0 {
                                None
                            } else {
                                Some(idx - 1)
                            }
                        }
                        BusState::AtStop(idx) => Some(idx),
                        BusState::DrivingOffMap => Some(r.stops.len() - 1),
                        BusState::Finished => unreachable!(),
                    };
                    (*bus, stop)
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// (buses, trains)
    pub fn active_vehicles(&self) -> (usize, usize) {
        let mut buses = 0;
        let mut trains = 0;
        for r in self.routes.values() {
            let len = r.active_vehicles.len();
            if len > 0 {
                if r.active_vehicles.iter().next().unwrap().vehicle_type == VehicleType::Bus {
                    buses += len;
                } else {
                    trains += len;
                }
            }
        }
        (buses, trains)
    }

    pub fn get_people_waiting_at_stop(
        &self,
        at: TransitStopID,
    ) -> &Vec<(PedestrianID, TransitRouteID, Option<TransitStopID>, Time)> {
        &self.peds_waiting[&at]
    }

    pub fn get_unzoomed_transit_riders(
        &self,
        now: Time,
        driving: &DrivingSimState,
        map: &Map,
    ) -> Vec<UnzoomedAgent> {
        let mut results = Vec::new();
        for (bus_id, bus) in &self.buses {
            if bus.passengers.is_empty() {
                continue;
            }
            let pos = if let Some(input) = driving.get_single_draw_car(*bus_id, now, map, self) {
                input.body.last_pt()
            } else {
                panic!(
                    "At {}, bus {} can't be drawn, yet it has passengers {:?}",
                    now, bus_id, bus.passengers
                );
            };
            for (person, _) in &bus.passengers {
                let agent = AgentID::BusPassenger(*person, *bus_id);
                results.push(UnzoomedAgent {
                    id: agent,
                    pos,
                    person: Some(*person),
                    parking: false,
                });
            }
        }
        results
    }
}

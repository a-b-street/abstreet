use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Time;
use map_model::{BusRoute, BusRouteID, BusStopID, Map, Path, PathRequest, Position};

use crate::sim::Ctx;
use crate::{
    CarID, Event, PedestrianID, PersonID, Router, TripID, TripManager, TripPhaseType, VehicleType,
    WalkingSimState,
};

// These index stops along a route, not stops along a single sidewalk.
type StopIdx = usize;

#[derive(Serialize, Deserialize, Clone)]
struct Stop {
    id: BusStopID,
    driving_pos: Position,
    next_stop: Option<Path>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Route {
    stops: Vec<Stop>,
    start: Path,
    end_at_border: Option<Path>,
    active_vehicles: BTreeSet<CarID>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Bus {
    car: CarID,
    route: BusRouteID,
    /// Where does each passenger want to deboard?
    passengers: Vec<(PersonID, Option<BusStopID>)>,
    state: BusState,
}

#[derive(Serialize, Deserialize, Clone)]
enum BusState {
    DrivingToStop(StopIdx),
    AtStop(StopIdx),
    DrivingOffMap,
    Done,
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
    routes: BTreeMap<BusRouteID, Route>,
    /// waiting at => (ped, route, bound for, started waiting)
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    peds_waiting: BTreeMap<BusStopID, Vec<(PedestrianID, BusRouteID, Option<BusStopID>, Time)>>,

    events: Vec<Event>,
}

impl TransitSimState {
    pub fn new(map: &Map) -> TransitSimState {
        // Keep this filled out always so get_passengers can return &Vec without a hassle
        let mut peds_waiting = BTreeMap::new();
        for bs in map.all_bus_stops().keys() {
            peds_waiting.insert(*bs, Vec::new());
        }

        TransitSimState {
            buses: BTreeMap::new(),
            routes: BTreeMap::new(),
            peds_waiting,
            events: Vec::new(),
        }
    }

    /// Returns the path for the first leg.
    pub fn create_empty_route(&mut self, bus_route: &BusRoute, map: &Map) -> Path {
        if !self.routes.contains_key(&bus_route.id) {
            assert!(bus_route.stops.len() > 1);
            let mut stops = Vec::new();
            for (idx, stop1_id) in bus_route.stops.iter().enumerate() {
                let stop1 = map.get_bs(*stop1_id);
                if idx == bus_route.stops.len() - 1 {
                    stops.push(Stop {
                        id: stop1.id,
                        driving_pos: stop1.driving_pos,
                        next_stop: None,
                    });
                    continue;
                }
                let req = PathRequest {
                    start: stop1.driving_pos,
                    end: map.get_bs(bus_route.stops[idx + 1]).driving_pos,
                    constraints: bus_route.route_type,
                };
                match map.pathfind(req) {
                    Ok(path) => {
                        if path.is_empty() {
                            panic!("Empty path between stops?! {}", path.get_req());
                        }
                        stops.push(Stop {
                            id: stop1.id,
                            driving_pos: stop1.driving_pos,
                            next_stop: Some(path),
                        });
                    }
                    Err(err) => {
                        panic!("No route between stops: {}", err);
                    }
                }
            }
            let start_req = PathRequest {
                start: Position::start(bus_route.start),
                end: map.get_bs(bus_route.stops[0]).driving_pos,
                constraints: bus_route.route_type,
            };
            let start = map.pathfind(start_req).expect("no route to first stop");
            let end_at_border = if let Some(l) = bus_route.end_border {
                let req = PathRequest {
                    start: map.get_bs(*bus_route.stops.last().unwrap()).driving_pos,
                    end: Position::end(l, map),
                    constraints: bus_route.route_type,
                };
                let path = map
                    .pathfind(req)
                    .expect("no route from last stop to border");
                Some(path)
            } else {
                None
            };
            self.routes.insert(
                bus_route.id,
                Route {
                    active_vehicles: BTreeSet::new(),
                    stops,
                    start,
                    end_at_border,
                },
            );
        }

        self.routes[&bus_route.id].start.clone()
    }

    pub fn bus_created(&mut self, bus: CarID, r: BusRouteID) {
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
                let stop1 = self.routes[&bus.route].stops[stop_idx].id;
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
                        self.events.push(Event::TripPhaseStarting(
                            trip,
                            person,
                            Some(PathRequest {
                                start: ctx.map.get_bs(stop1).driving_pos,
                                end: if let Some(stop2) = maybe_stop2 {
                                    ctx.map.get_bs(stop2).driving_pos
                                } else {
                                    self.routes[&route]
                                        .end_at_border
                                        .as_ref()
                                        .unwrap()
                                        .get_req()
                                        .end
                                },
                                constraints: bus.car.1.to_constraints(),
                            }),
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
                bus.state = BusState::Done;
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
                false
            }
            BusState::AtStop(_) | BusState::Done => unreachable!(),
        }
    }

    pub fn bus_departed_from_stop(&mut self, id: CarID, map: &Map) -> Router {
        let mut bus = self.buses.get_mut(&id).unwrap();
        let route = self.routes.get_mut(&bus.route).unwrap();
        match bus.state {
            BusState::DrivingToStop(_) | BusState::DrivingOffMap | BusState::Done => unreachable!(),
            BusState::AtStop(stop_idx) => {
                let stop = &route.stops[stop_idx];
                self.events
                    .push(Event::BusDepartedFromStop(id, bus.route, stop.id));
                if let Some(path) = stop.next_stop.clone() {
                    bus.state = BusState::DrivingToStop(stop_idx + 1);
                    Router::follow_bus_route(id, path)
                } else {
                    if let Some(path) = route.end_at_border.clone() {
                        bus.state = BusState::DrivingOffMap;
                        Router::follow_bus_route(id, path)
                    } else {
                        route.active_vehicles.remove(&id);
                        for (person, stop2) in &bus.passengers {
                            panic!(
                                "{} of {} is vanishing at its last stop, but {} is still riding \
                                 until {:?}",
                                bus.car, bus.route, person, stop2
                            );
                        }
                        bus.state = BusState::Done;
                        Router::vanish_bus(id, stop.driving_pos, map)
                    }
                }
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
        stop1: BusStopID,
        route_id: BusRouteID,
        maybe_stop2: Option<BusStopID>,
        map: &Map,
    ) -> Option<CarID> {
        assert!(Some(stop1) != maybe_stop2);
        if let Some(route) = self.routes.get(&route_id) {
            for bus in &route.active_vehicles {
                if let BusState::AtStop(idx) = self.buses[bus].state {
                    if route.stops[idx].id == stop1 {
                        self.buses
                            .get_mut(bus)
                            .unwrap()
                            .passengers
                            .push((person, maybe_stop2));
                        self.events.push(Event::TripPhaseStarting(
                            trip,
                            person,
                            Some(PathRequest {
                                start: map.get_bs(stop1).driving_pos,
                                end: if let Some(stop2) = maybe_stop2 {
                                    map.get_bs(stop2).driving_pos
                                } else {
                                    route.end_at_border.as_ref().unwrap().get_req().end
                                },
                                constraints: bus.1.to_constraints(),
                            }),
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

    pub fn get_passengers(&self, bus: CarID) -> &Vec<(PersonID, Option<BusStopID>)> {
        &self.buses[&bus].passengers
    }

    pub fn bus_route(&self, bus: CarID) -> BusRouteID {
        self.buses[&bus].route
    }

    /// also stop idx that the bus is coming from
    pub fn buses_for_route(&self, route: BusRouteID) -> Vec<(CarID, Option<usize>)> {
        if let Some(ref r) = self.routes.get(&route) {
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
                        BusState::Done => unreachable!(),
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
                if r.active_vehicles.iter().next().unwrap().1 == VehicleType::Bus {
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
        at: BusStopID,
    ) -> &Vec<(PedestrianID, BusRouteID, Option<BusStopID>, Time)> {
        &self.peds_waiting[&at]
    }
}

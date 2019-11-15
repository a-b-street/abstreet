use crate::{CarID, Event, PedestrianID, Router, Scheduler, TripManager, WalkingSimState};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Distance, Duration};
use map_model::{
    BusRoute, BusRouteID, BusStopID, Map, Path, PathConstraints, PathRequest, Position,
};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

// These index stops along a route, not stops along a single sidewalk.
type StopIdx = usize;

#[derive(Serialize, Deserialize, PartialEq)]
struct StopForRoute {
    id: BusStopID,
    driving_pos: Position,
    path_to_next_stop: Path,
    next_stop_idx: StopIdx,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct Route {
    stops: Vec<StopForRoute>,
    buses: Vec<CarID>,
}

#[derive(Serialize, Deserialize, PartialEq)]
struct Bus {
    car: CarID,
    route: BusRouteID,
    // Where does each passenger want to deboard?
    passengers: Vec<(PedestrianID, BusStopID)>,
    state: BusState,
}

#[derive(Serialize, Deserialize, PartialEq)]
enum BusState {
    DrivingToStop(StopIdx),
    AtStop(StopIdx),
}

// This kind of acts like TripManager, managing transitions... but a bit more statefully.
#[derive(Serialize, Deserialize, PartialEq)]
pub struct TransitSimState {
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
    // Can organize this more to make querying cheaper
    peds_waiting: Vec<(PedestrianID, BusStopID, BusRouteID, BusStopID)>,

    events: Vec<Event>,
}

impl TransitSimState {
    pub fn new() -> TransitSimState {
        TransitSimState {
            buses: BTreeMap::new(),
            routes: BTreeMap::new(),
            peds_waiting: Vec::new(),
            events: Vec::new(),
        }
    }

    // Returns (next stop, first path, end distance for next stop) for all of the stops in the
    // route.
    pub fn create_empty_route(
        &mut self,
        bus_route: &BusRoute,
        map: &Map,
    ) -> Vec<(StopIdx, Path, Distance)> {
        assert!(bus_route.stops.len() > 1);

        let route = Route {
            buses: Vec::new(),
            stops: bus_route
                .stops
                .iter()
                .enumerate()
                .map(|(idx, stop1_id)| {
                    let stop1 = map.get_bs(*stop1_id);
                    let stop2_idx = if idx + 1 == bus_route.stops.len() {
                        0
                    } else {
                        idx + 1
                    };
                    let path = map
                        .pathfind(PathRequest {
                            start: stop1.driving_pos,
                            end: map.get_bs(bus_route.stops[stop2_idx]).driving_pos,
                            constraints: PathConstraints::Bus,
                        })
                        .expect(&format!(
                            "No route between bus stops {:?} and {:?}",
                            stop1_id, bus_route.stops[stop2_idx]
                        ));
                    StopForRoute {
                        id: *stop1_id,
                        driving_pos: stop1.driving_pos,
                        path_to_next_stop: path,
                        next_stop_idx: stop2_idx,
                    }
                })
                .collect(),
        };

        let stops = route
            .stops
            .iter()
            .map(|s| {
                (
                    s.next_stop_idx,
                    s.path_to_next_stop.clone(),
                    route.stops[s.next_stop_idx].driving_pos.dist_along(),
                )
            })
            .collect();
        self.routes.insert(bus_route.id, route);
        stops
    }

    pub fn bus_created(&mut self, bus: CarID, route: BusRouteID, next_stop_idx: StopIdx) {
        self.routes.get_mut(&route).unwrap().buses.push(bus);
        self.buses.insert(
            bus,
            Bus {
                car: bus,
                route,
                passengers: Vec::new(),
                state: BusState::DrivingToStop(next_stop_idx),
            },
        );
    }

    pub fn bus_arrived_at_stop(
        &mut self,
        now: Duration,
        id: CarID,
        trips: &mut TripManager,
        walking: &mut WalkingSimState,
        scheduler: &mut Scheduler,
        map: &Map,
    ) {
        let mut bus = self.buses.get_mut(&id).unwrap();
        match bus.state {
            BusState::DrivingToStop(stop_idx) => {
                bus.state = BusState::AtStop(stop_idx);
                let stop = self.routes[&bus.route].stops[stop_idx].id;
                self.events
                    .push(Event::BusArrivedAtStop(id, bus.route, stop));

                // Deboard existing passengers.
                let mut still_riding = Vec::new();
                for (ped, stop2) in bus.passengers.drain(..) {
                    if stop == stop2 {
                        self.events.push(Event::PedLeavesBus(ped, id, bus.route));
                        trips.ped_left_bus(now, ped, map, scheduler);
                    } else {
                        still_riding.push((ped, stop2));
                    }
                }
                bus.passengers = still_riding;

                // Board new passengers.
                let mut still_waiting = Vec::new();
                for (ped, stop1, route, stop2) in self.peds_waiting.drain(..) {
                    if stop == stop1 && bus.route == route {
                        bus.passengers.push((ped, stop2));
                        self.events.push(Event::PedEntersBus(ped, id, route));
                        trips.ped_boarded_bus(ped, walking);
                    } else {
                        still_waiting.push((ped, stop1, route, stop2));
                    }
                }
                self.peds_waiting = still_waiting;
            }
            BusState::AtStop(_) => unreachable!(),
        };
    }

    pub fn bus_departed_from_stop(&mut self, id: CarID) -> Router {
        let mut bus = self.buses.get_mut(&id).unwrap();
        match bus.state {
            BusState::DrivingToStop(_) => unreachable!(),
            BusState::AtStop(stop_idx) => {
                let route = &self.routes[&bus.route];
                let stop = &route.stops[stop_idx];

                bus.state = BusState::DrivingToStop(stop.next_stop_idx);
                self.events
                    .push(Event::BusDepartedFromStop(id, bus.route, stop.id));
                Router::follow_bus_route(
                    stop.path_to_next_stop.clone(),
                    route.stops[stop.next_stop_idx].driving_pos.dist_along(),
                )
            }
        }
    }

    // If true, the pedestrian boarded a bus immediately.
    pub fn ped_waiting_for_bus(
        &mut self,
        ped: PedestrianID,
        stop1: BusStopID,
        route_id: BusRouteID,
        stop2: BusStopID,
    ) -> bool {
        assert!(stop1 != stop2);
        let route = &self.routes[&route_id];
        for bus in &route.buses {
            if let BusState::AtStop(idx) = self.buses[bus].state {
                if route.stops[idx].id == stop1 {
                    self.buses
                        .get_mut(bus)
                        .unwrap()
                        .passengers
                        .push((ped, stop2));
                    // TODO shift trips
                    self.events.push(Event::PedEntersBus(ped, *bus, route_id));
                    return true;
                }
            }
        }

        self.peds_waiting.push((ped, stop1, route_id, stop2));
        false
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    pub fn get_passengers(&self, bus: CarID) -> &Vec<(PedestrianID, BusStopID)> {
        &self.buses[&bus].passengers
    }

    pub fn bus_route(&self, bus: CarID) -> BusRouteID {
        self.buses[&bus].route
    }

    pub fn buses_for_route(&self, route: BusRouteID) -> Vec<CarID> {
        if let Some(ref r) = self.routes.get(&route) {
            r.buses.clone()
        } else {
            Vec::new()
        }
    }
}

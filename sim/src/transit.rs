use crate::{
    CarID, Command, Event, PedestrianID, PriorityQueue, Router, TripManager, WalkingSimState,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Distance, Duration};
use map_model::{BusRoute, BusRouteID, BusStop, BusStopID, Map, Path, PathRequest, Pathfinder};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

// These index stops along a route, not stops along a single sidewalk.
type StopIdx = usize;

#[derive(Serialize, Deserialize, PartialEq)]
struct Route {
    // Just copy the info over here from map_model for convenience
    id: BusRouteID,
    name: String,
    stops: Vec<BusStop>,

    buses: Vec<CarID>,
    // TODO info on schedules
}

impl Route {
    fn next_stop(&self, idx: StopIdx) -> StopIdx {
        if idx + 1 == self.stops.len() {
            0
        } else {
            idx + 1
        }
    }
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

    // Returns (next stop, start distance on the driving lane, first path, end distance for next
    // stop) for all of the stops in the route.
    pub fn create_empty_route(
        &mut self,
        route: &BusRoute,
        map: &Map,
    ) -> Vec<(StopIdx, Distance, Path, Distance)> {
        assert!(route.stops.len() > 1);
        let route = Route {
            id: route.id,
            name: route.name.clone(),
            stops: route.stops.iter().map(|s| map.get_bs(*s).clone()).collect(),
            buses: Vec::new(),
        };

        let stops = route
            .stops
            .iter()
            .enumerate()
            .map(|(idx, stop1)| {
                let next_stop = route.next_stop(idx);
                let stop2 = &route.stops[next_stop];
                let path = Pathfinder::shortest_distance(
                    map,
                    PathRequest {
                        start: stop1.driving_pos,
                        end: stop2.driving_pos,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: true,
                    },
                )
                .expect(&format!(
                    "No route between bus stops {:?} and {:?}",
                    stop1, stop2
                ));
                (
                    next_stop,
                    stop1.driving_pos.dist_along(),
                    path,
                    stop2.driving_pos.dist_along(),
                )
            })
            .collect();

        self.routes.insert(route.id, route);
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
        time: Duration,
        id: CarID,
        trips: &mut TripManager,
        walking: &mut WalkingSimState,
        scheduler: &mut PriorityQueue<Command>,
        map: &Map,
    ) {
        let mut bus = self.buses.get_mut(&id).unwrap();
        match bus.state {
            BusState::DrivingToStop(stop_idx) => {
                bus.state = BusState::AtStop(stop_idx);
                let stop = self.routes[&bus.route].stops[stop_idx].id;
                self.events.push(Event::BusArrivedAtStop(id, stop));

                // Deboard existing passengers.
                let mut still_riding = Vec::new();
                for (ped, stop2) in bus.passengers.drain(..) {
                    if stop == stop2 {
                        self.events.push(Event::PedLeavesBus(ped, id));
                        trips.ped_left_bus(time, ped, map, scheduler);
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
                        self.events.push(Event::PedEntersBus(ped, id));
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

    pub fn bus_departed_from_stop(&mut self, id: CarID, map: &Map) -> Router {
        let mut bus = self.buses.get_mut(&id).unwrap();
        match bus.state {
            BusState::DrivingToStop(_) => unreachable!(),
            BusState::AtStop(stop_idx) => {
                let route = &self.routes[&bus.route];
                let next_stop_idx = route.next_stop(stop_idx);
                let stop = &route.stops[stop_idx];
                let next_stop = &route.stops[next_stop_idx];
                bus.state = BusState::DrivingToStop(next_stop_idx);
                self.events.push(Event::BusDepartedFromStop(id, stop.id));

                let new_path = Pathfinder::shortest_distance(
                    map,
                    PathRequest {
                        start: stop.driving_pos,
                        end: next_stop.driving_pos,
                        can_use_bike_lanes: false,
                        can_use_bus_lanes: true,
                    },
                )
                .expect(&format!(
                    "No route between bus stops {:?} and {:?}",
                    stop, next_stop
                ));
                Router::follow_bus_route(new_path, next_stop.driving_pos.dist_along())
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
                    self.events.push(Event::PedEntersBus(ped, *bus));
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
}

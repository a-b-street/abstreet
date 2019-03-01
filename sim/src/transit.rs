use crate::{CarID, Event, PedestrianID, Router};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Distance;
use map_model::{BusRoute, BusRouteID, BusStop, Map, Path, PathRequest, Pathfinder};
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
    passengers: Vec<PedestrianID>,
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

    events: Vec<Event>,
}

impl TransitSimState {
    pub fn new() -> TransitSimState {
        TransitSimState {
            buses: BTreeMap::new(),
            routes: BTreeMap::new(),
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

    pub fn bus_arrived_at_stop(&mut self, id: CarID) {
        let mut bus = self.buses.get_mut(&id).unwrap();
        match bus.state {
            BusState::DrivingToStop(stop_idx) => {
                bus.state = BusState::AtStop(stop_idx);
                self.events.push(Event::BusArrivedAtStop(
                    id,
                    self.routes[&bus.route].stops[stop_idx].id,
                ));
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

    pub fn collect_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    /*pub fn step(
        &mut self,
        now: Tick,
        events: &mut Vec<Event>,
        walking_sim: &mut WalkingSimState,
        trips: &mut TripManager,
        spawner: &mut Spawner,
        map: &Map,
    ) {
        for b in self.buses.values_mut() {
            if let BusState::AtStop(stop_idx, _) = b.state {
                let stop = &self.routes[&b.route].stops[stop_idx];

                // Let anybody new on?
                for p in walking_sim.get_peds_waiting_at_stop(stop.id).into_iter() {
                    if trips.should_ped_board_bus(p, b.route) {
                        events.push(Event::PedEntersBus(p, b.car));
                        b.passengers.push(p);
                        walking_sim.ped_joined_bus(p, stop.id);
                    }
                }

                // Let anybody off?
                // TODO ideally dont even ask if they just got on, but the trip planner things
                // should be fine with this
                // TODO only do this if we JUST arrived at the stop, and in fact, wait for everyone
                // to leave, since it may take time.
                // so actually, we shouldnt statechange mutably in get_action_when_stopped_at_end,
                // which is called by router! thats convoluted
                let car = b.car;
                b.passengers.retain(|p| {
                    if trips.should_ped_leave_bus(*p, stop.id) {
                        events.push(Event::PedLeavesBus(*p, car));
                        // TODO would be a little cleaner to return this info up to sim and have it
                        // plumb through to spawner? not sure
                        spawner.ped_finished_bus_ride(now, *p, stop.id, trips, map);
                        false
                    } else {
                        true
                    }
                });
            }
        }
    }*/
}

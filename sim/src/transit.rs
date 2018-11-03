use abstutil::{deserialize_btreemap, serialize_btreemap};
use dimensioned::si;
use events::Event;
use instrument::capture_backtrace;
use map_model::{BusRoute, BusStop, LaneID, Map, Path, Pathfinder};
use spawn::Spawner;
use std::collections::BTreeMap;
use trips::TripManager;
use view::AgentView;
use walking::WalkingSimState;
use {CarID, Distance, PedestrianID, RouteID, Tick};

// These index stops along a route, not stops along a single sidewalk.
type StopIdx = usize;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Route {
    id: RouteID,
    name: String,
    buses: Vec<CarID>,
    // Just copy the info over here for convenience
    stops: Vec<BusStop>,
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

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Bus {
    car: CarID,
    route: RouteID,
    passengers: Vec<PedestrianID>,
    state: BusState,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
enum BusState {
    DrivingToStop(StopIdx),
    // When do we leave?
    AtStop(StopIdx, Tick),
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct TransitSimState {
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    buses: BTreeMap<CarID, Bus>,
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
    routes: BTreeMap<RouteID, Route>,
}

impl TransitSimState {
    pub fn new() -> TransitSimState {
        TransitSimState {
            buses: BTreeMap::new(),
            routes: BTreeMap::new(),
        }
    }

    pub fn create_empty_route(&mut self, route: &BusRoute, map: &Map) -> RouteID {
        assert!(route.stops.len() > 1);
        let id = RouteID(self.routes.len());
        self.routes.insert(
            id,
            Route {
                id,
                name: route.name.clone(),
                buses: Vec::new(),
                stops: route.stops.iter().map(|s| map.get_bs(*s).clone()).collect(),
            },
        );
        id
    }

    // (next stop, start distance, first path)
    pub fn get_route_starts(&self, id: RouteID, map: &Map) -> Vec<(StopIdx, Distance, Path)> {
        let route = &self.routes[&id];
        route
            .stops
            .iter()
            .enumerate()
            .map(|(idx, stop1)| {
                let next_stop = route.next_stop(idx);
                let stop2 = &route.stops[next_stop];
                let path = Pathfinder::shortest_distance(
                    map,
                    stop1.driving_lane,
                    stop1.dist_along,
                    stop2.driving_lane,
                    stop2.dist_along,
                    false,
                ).expect(&format!(
                    "No route between bus stops {:?} and {:?}",
                    stop1, stop2
                ));
                (next_stop, stop1.dist_along, path)
            }).collect()
    }

    pub fn bus_created(&mut self, bus: CarID, route: RouteID, next_stop_idx: StopIdx) {
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

    // Returns (should idle, new path)
    pub fn get_action_when_stopped_at_end(
        &mut self,
        events: &mut Vec<Event>,
        view: &AgentView,
        time: Tick,
        map: &Map,
    ) -> (bool, Option<Path>) {
        let car = view.id.as_car();
        let route = &self.routes[&self.buses[&car].route];
        match self.buses[&car].state {
            BusState::DrivingToStop(stop_idx) => {
                let stop = &route.stops[stop_idx];
                assert_eq!(stop.driving_lane, view.on.as_lane());
                if stop.dist_along == view.dist_along {
                    // TODO constant for stop time
                    self.buses.get_mut(&car).unwrap().state =
                        BusState::AtStop(stop_idx, time + 10.0 * si::S);
                    events.push(Event::BusArrivedAtStop(car, stop.id));
                    capture_backtrace("BusArrivedAtStop");
                    if view.debug {
                        debug!("{} arrived at stop {:?}, now waiting", car, stop);
                    }
                    return (true, None);
                }
                // No, keep creeping forwards
                (false, None)
            }
            BusState::AtStop(stop_idx, wait_until) => {
                let stop = &route.stops[stop_idx];
                assert_eq!(stop.driving_lane, view.on.as_lane());
                assert_eq!(stop.dist_along, view.dist_along);

                if time == wait_until {
                    let next_stop = route.next_stop(stop_idx);
                    self.buses.get_mut(&car).unwrap().state = BusState::DrivingToStop(next_stop);
                    events.push(Event::BusDepartedFromStop(car, stop.id));
                    capture_backtrace("BusDepartedFromStop");
                    if view.debug {
                        debug!("{} departing from stop {:?}", car, stop);
                    }

                    let new_path = Pathfinder::shortest_distance(
                        map,
                        stop.driving_lane,
                        stop.dist_along,
                        route.stops[next_stop].driving_lane,
                        route.stops[next_stop].dist_along,
                        false,
                    ).expect(&format!(
                        "No route between bus stops {:?} and {:?}",
                        stop, route.stops[next_stop]
                    ));

                    return (true, Some(new_path));
                }

                (true, None)
            }
        }
    }

    pub fn get_dist_to_stop_at(&self, bus: CarID, driving_lane: LaneID) -> Distance {
        match self.buses[&bus].state {
            BusState::DrivingToStop(stop_idx) => {
                let stop = &self.routes[&self.buses[&bus].route].stops[stop_idx];
                assert_eq!(stop.driving_lane, driving_lane);
                stop.dist_along
            }
            BusState::AtStop(_, _) => {
                panic!("Shouldn't ask where to stop if the bus is already at a stop")
            }
        }
    }

    pub fn step(
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
                        capture_backtrace("PedEntersBus");
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
                        capture_backtrace("PedLeavesBus");
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
    }
}

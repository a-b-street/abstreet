use crate::{
    CarID, Event, ParkingSimState, PedestrianID, PersonID, Router, Scheduler, TripID, TripManager,
    TripPhaseType, VehicleType, WalkingSimState,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Time;
use map_model::{BusRoute, BusRouteID, BusStopID, Map, Path, PathRequest, Position};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

// These index stops along a route, not stops along a single sidewalk.
type StopIdx = usize;

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct Stop {
    id: BusStopID,
    driving_pos: Position,
    next_stop: Option<(PathRequest, Path)>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct Route {
    stops: Vec<Stop>,
    start_from_border: Option<(PathRequest, Path)>,
    end_at_border: Option<(PathRequest, Path)>,
    active_vehicles: BTreeSet<CarID>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
struct Bus {
    car: CarID,
    route: BusRouteID,
    // Where does each passenger want to deboard?
    passengers: Vec<(PersonID, Option<BusStopID>)>,
    state: BusState,
}

#[derive(Serialize, Deserialize, PartialEq, Clone)]
enum BusState {
    DrivingToStop(StopIdx),
    AtStop(StopIdx),
    DrivingOffMap,
    Done,
}

// This kind of acts like TripManager, managing transitions... but a bit more statefully.
#[derive(Serialize, Deserialize, PartialEq, Clone)]
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
    // waiting at => (ped, route, bound for, started waiting)
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    peds_waiting: BTreeMap<BusStopID, Vec<(PedestrianID, BusRouteID, Option<BusStopID>, Time)>>,

    events: Vec<Event>,
}

impl TransitSimState {
    pub fn new() -> TransitSimState {
        TransitSimState {
            buses: BTreeMap::new(),
            routes: BTreeMap::new(),
            peds_waiting: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    // Returns the path for the first leg.
    pub fn create_empty_route(&mut self, bus_route: &BusRoute, map: &Map) -> (PathRequest, Path) {
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
            if let Some(path) = map.pathfind(req.clone()) {
                if path.is_empty() {
                    panic!("Empty path between stops?! {}", req);
                }
                stops.push(Stop {
                    id: stop1.id,
                    driving_pos: stop1.driving_pos,
                    next_stop: Some((req, path)),
                });
            } else {
                panic!("No route between stops: {}", req);
            }
        }
        let start_from_border = if let Some(l) = bus_route.start_border {
            let req = PathRequest {
                start: Position::start(l),
                end: map.get_bs(bus_route.stops[0]).driving_pos,
                constraints: bus_route.route_type,
            };
            let path = map
                .pathfind(req.clone())
                .expect("no route from border to first stop");
            Some((req, path))
        } else {
            None
        };
        let end_at_border = if let Some(l) = bus_route.end_border {
            let req = PathRequest {
                start: map.get_bs(*bus_route.stops.last().unwrap()).driving_pos,
                end: Position::end(l, map),
                constraints: bus_route.route_type,
            };
            let path = map
                .pathfind(req.clone())
                .expect("no route from last stop to border");
            Some((req, path))
        } else {
            None
        };

        let first_step = start_from_border
            .clone()
            .or(stops[0].next_stop.clone())
            .unwrap();
        self.routes.insert(
            bus_route.id,
            Route {
                active_vehicles: BTreeSet::new(),
                stops,
                start_from_border,
                end_at_border,
            },
        );
        first_step
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
                state: if route.start_from_border.is_some() {
                    BusState::DrivingToStop(0)
                } else {
                    BusState::DrivingToStop(1)
                },
            },
        );
    }

    // If true, the bus is idling. If false, the bus actually arrived at a border and should now
    // vanish.
    pub fn bus_arrived_at_stop(
        &mut self,
        now: Time,
        id: CarID,
        trips: &mut TripManager,
        walking: &mut WalkingSimState,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
        map: &Map,
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
                        trips.person_left_bus(now, person, bus.car, map, scheduler);
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
                    self.peds_waiting.remove(&stop1).unwrap_or_else(Vec::new)
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
                                start: map.get_bs(stop1).driving_pos,
                                end: if let Some(stop2) = maybe_stop2 {
                                    map.get_bs(stop2).driving_pos
                                } else {
                                    self.routes[&route].end_at_border.as_ref().unwrap().0.end
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
                        // TODO Pre-existing bug...
                        println!(
                            "{} fell asleep on {} and just rode off-map, but they were supposed \
                             to hop off at {}",
                            person, bus.car, stop2
                        );
                        continue;
                    }
                    trips.transit_rider_reached_border(now, person, id, map, parking, scheduler);
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
                if let Some((req, path)) = stop.next_stop.clone() {
                    bus.state = BusState::DrivingToStop(stop_idx + 1);
                    Router::follow_bus_route(id, path, req.end.dist_along())
                } else {
                    if let Some((req, path)) = route.end_at_border.clone() {
                        bus.state = BusState::DrivingOffMap;
                        Router::follow_bus_route(id, path, req.end.dist_along())
                    } else {
                        let on = stop.driving_pos.lane();
                        route.active_vehicles.remove(&id);
                        assert!(bus.passengers.is_empty());
                        bus.state = BusState::Done;
                        Router::vanish_bus(id, on, map)
                    }
                }
            }
        }
    }

    // Returns the bus if the pedestrian boarded immediately.
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
                                    route.end_at_border.as_ref().unwrap().0.end
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
            .entry(stop1)
            .or_insert_with(Vec::new)
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

    // also stop idx that the bus is coming from
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

    // (buses, trains)
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
}

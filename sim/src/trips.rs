use std::collections::{BTreeMap, VecDeque};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap, Counter};
use geom::{Distance, Duration, Speed, Time};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, Map, Path, PathConstraints, PathRequest,
    Position,
};

use crate::cap::CapResult;
use crate::sim::Ctx;
use crate::{
    AgentID, AgentType, AlertLocation, CarID, Command, CreateCar, CreatePedestrian, DrivingGoal,
    Event, IndividTrip, OrigPersonID, ParkedCar, ParkingSim, ParkingSpot, PedestrianID, PersonID,
    PersonSpec, Scenario, SidewalkPOI, SidewalkSpot, TransitSimState, TripEndpoint, TripID,
    TripPhaseType, TripPurpose, TripSpec, Vehicle, VehicleSpec, VehicleType, WalkingSimState,
};

/// Manages people, each of which executes some trips through the day. Each trip is further broken
/// down into legs -- for example, a driving trip might start with somebody walking to their car,
/// driving somewhere, parking, and then walking to their final destination.
/// https://dabreegster.github.io/abstreet/trafficsim/trips.html describes some of the variations.
//
// Here be dragons, keep hands and feet inside the ride at all times...
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct TripManager {
    trips: Vec<Trip>,
    people: Vec<Person>,
    // For quick lookup of active agents
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    active_trip_mode: BTreeMap<AgentID, TripID>,
    unfinished_trips: usize,

    car_id_counter: usize,

    events: Vec<Event>,
}

// Initialization
impl TripManager {
    pub fn new() -> TripManager {
        TripManager {
            trips: Vec::new(),
            people: Vec::new(),
            active_trip_mode: BTreeMap::new(),
            unfinished_trips: 0,
            car_id_counter: 0,
            events: Vec::new(),
        }
    }

    // TODO assert the specs are correct yo
    pub fn new_person(
        &mut self,
        orig_id: Option<OrigPersonID>,
        ped_speed: Speed,
        vehicle_specs: Vec<VehicleSpec>,
    ) -> &Person {
        let id = PersonID(self.people.len());
        let vehicles = vehicle_specs
            .into_iter()
            .map(|v| {
                let c = CarID(self.new_car_id(), v.vehicle_type);
                v.make(c, Some(id))
            })
            .collect();
        self.people.push(Person {
            id,
            orig_id,
            trips: Vec::new(),
            // The first new_trip will set this properly.
            state: PersonState::OffMap,
            ped: PedestrianID(id.0),
            ped_speed,
            vehicles,
            delayed_trips: Vec::new(),
            on_bus: None,
        });
        self.get_person(id).unwrap()
    }

    pub fn new_car_id(&mut self) -> usize {
        let id = self.car_id_counter;
        self.car_id_counter += 1;
        id
    }

    pub fn new_trip(&mut self, person: PersonID, info: TripInfo, legs: Vec<TripLeg>) -> TripID {
        assert!(!legs.is_empty());
        // TODO Make sure the legs constitute a valid state machine.

        let id = TripID(self.trips.len());
        let trip = Trip {
            id,
            info,
            person,
            started: false,
            finished_at: None,
            total_blocked_time: Duration::ZERO,
            total_distance: Distance::ZERO,
            legs: VecDeque::from(legs),
        };
        self.unfinished_trips += 1;
        let person = &mut self.people[trip.person.0];
        if person.trips.is_empty() {
            person.state = match trip.info.start {
                TripEndpoint::Bldg(b) => {
                    self.events
                        .push(Event::PersonEntersBuilding(trip.person, b));
                    PersonState::Inside(b)
                }
                TripEndpoint::Border(_) | TripEndpoint::SuddenlyAppear(_) => PersonState::OffMap,
            };
        }
        if let Some(t) = person.trips.last() {
            // TODO If it's exactly ==, what?! See the ID.
            if self.trips[t.0].info.departure > trip.info.departure {
                panic!(
                    "{} has a trip starting at {}, then one at {}",
                    person.id, self.trips[t.0].info.departure, trip.info.departure
                );
            }
        }
        person.trips.push(id);
        self.trips.push(trip);
        id
    }

    pub fn start_trip(&mut self, now: Time, trip: TripID, spec: TripSpec, ctx: &mut Ctx) {
        assert!(self.trips[trip.0].info.cancellation_reason.is_none());

        let person = &mut self.people[self.trips[trip.0].person.0];
        if let PersonState::Trip(_) = person.state {
            // Previous trip isn't done. Defer this one!
            if false {
                self.events.push(Event::Alert(
                    AlertLocation::Person(person.id),
                    format!(
                        "{} is still doing a trip, so not starting {} yet",
                        person.id, trip
                    ),
                ));
            }
            person.delayed_trips.push((trip, spec));
            self.events.push(Event::TripPhaseStarting(
                trip,
                person.id,
                None,
                TripPhaseType::DelayedStart,
            ));
            return;
        }
        self.trips[trip.0].started = true;

        match spec {
            TripSpec::VehicleAppearing {
                start_pos,
                goal,
                retry_if_no_room,
                use_vehicle,
            } => {
                assert_eq!(person.state, PersonState::OffMap);
                self.events.push(Event::PersonEntersMap(
                    person.id,
                    AgentID::Car(use_vehicle),
                    ctx.map.get_l(start_pos.lane()).src_i,
                ));
                person.state = PersonState::Trip(trip);

                let vehicle = person.get_vehicle(use_vehicle);
                assert!(ctx.parking.lookup_parked_car(vehicle.id).is_none());
                let constraints = if use_vehicle.1 == VehicleType::Bike {
                    PathConstraints::Bike
                } else {
                    PathConstraints::Car
                };
                let req = PathRequest {
                    start: start_pos,
                    end: goal.goal_pos(constraints, ctx.map).unwrap(),
                    constraints,
                };
                let person = person.id;

                match self.maybe_spawn_car(ctx, now, trip, req, vehicle.id) {
                    Ok(path) => {
                        let router = goal.make_router(vehicle.id, path, ctx.map);
                        ctx.scheduler.push(
                            now,
                            Command::SpawnCar(
                                CreateCar::for_appearing(vehicle, router, trip, person),
                                retry_if_no_room,
                            ),
                        );
                    }
                    Err(err) => {
                        self.cancel_trip(now, trip, err.to_string(), Some(vehicle), ctx);
                    }
                }
            }
            TripSpec::SpawningFailure {
                use_vehicle, error, ..
            } => {
                let vehicle = use_vehicle.map(|v| person.get_vehicle(v));
                self.cancel_trip(now, trip, error, vehicle, ctx);
            }
            TripSpec::UsingParkedCar {
                car, start_bldg, ..
            } => {
                assert_eq!(person.state, PersonState::Inside(start_bldg));
                person.state = PersonState::Trip(trip);

                if let Some(parked_car) = ctx.parking.lookup_parked_car(car).cloned() {
                    let start = SidewalkSpot::building(start_bldg, ctx.map);
                    let walking_goal =
                        SidewalkSpot::parking_spot(parked_car.spot, ctx.map, ctx.parking);
                    let req = PathRequest {
                        start: start.sidewalk_pos,
                        end: walking_goal.sidewalk_pos,
                        constraints: PathConstraints::Pedestrian,
                    };
                    match ctx.map.pathfind(req) {
                        Ok(path) => {
                            ctx.scheduler.push(
                                now,
                                Command::SpawnPed(CreatePedestrian {
                                    id: person.ped,
                                    speed: person.ped_speed,
                                    start,
                                    goal: walking_goal,
                                    path,
                                    trip,
                                    person: person.id,
                                }),
                            );
                        }
                        Err(err) => {
                            // Move the car to the destination
                            ctx.parking.remove_parked_car(parked_car.clone());
                            self.cancel_trip(
                                now,
                                trip,
                                err.to_string(),
                                Some(parked_car.vehicle),
                                ctx,
                            );
                        }
                    }
                } else {
                    // This should only happen when a driving trip has been cancelled and there was
                    // absolutely no room to warp the car.
                    self.cancel_trip(
                        now,
                        trip,
                        format!("should have {} parked somewhere, but it's unavailable", car),
                        None,
                        ctx,
                    );
                }
            }
            TripSpec::JustWalking { start, goal } => {
                assert_eq!(
                    person.state,
                    match start.connection {
                        SidewalkPOI::Building(b) => PersonState::Inside(b),
                        SidewalkPOI::Border(i) => {
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                i,
                            ));
                            PersonState::OffMap
                        }
                        SidewalkPOI::SuddenlyAppear => {
                            // Unclear which end of the sidewalk this person should be associated
                            // with. For interactively spawned people, doesn't really matter.
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                ctx.map.get_l(start.sidewalk_pos.lane()).src_i,
                            ));
                            PersonState::OffMap
                        }
                        _ => unreachable!(),
                    }
                );
                person.state = PersonState::Trip(trip);

                let req = PathRequest {
                    start: start.sidewalk_pos,
                    end: goal.sidewalk_pos,
                    constraints: PathConstraints::Pedestrian,
                };
                match ctx.map.pathfind(req) {
                    Ok(path) => {
                        ctx.scheduler.push(
                            now,
                            Command::SpawnPed(CreatePedestrian {
                                id: person.ped,
                                speed: person.ped_speed,
                                start,
                                goal,
                                path,
                                trip,
                                person: person.id,
                            }),
                        );
                    }
                    Err(err) => {
                        self.cancel_trip(now, trip, err.to_string(), None, ctx);
                    }
                }
            }
            TripSpec::UsingBike { start, .. } => {
                assert_eq!(person.state, PersonState::Inside(start));
                person.state = PersonState::Trip(trip);

                if let Some(walk_to) = SidewalkSpot::bike_rack(start, ctx.map) {
                    let req = PathRequest {
                        start: SidewalkSpot::building(start, ctx.map).sidewalk_pos,
                        end: walk_to.sidewalk_pos,
                        constraints: PathConstraints::Pedestrian,
                    };
                    match ctx.map.pathfind(req) {
                        Ok(path) => {
                            // Where we start biking may have slightly changed due to live map
                            // edits!
                            match self.trips[trip.0].legs.front_mut() {
                                Some(TripLeg::Walk(ref mut spot)) => {
                                    if spot.clone() != walk_to {
                                        // We could assert both have a BikeRack connection, but eh
                                        *spot = walk_to.clone();
                                    }
                                }
                                _ => unreachable!(),
                            }

                            ctx.scheduler.push(
                                now,
                                Command::SpawnPed(CreatePedestrian {
                                    id: person.ped,
                                    speed: person.ped_speed,
                                    start: SidewalkSpot::building(start, ctx.map),
                                    goal: walk_to,
                                    path,
                                    trip,
                                    person: person.id,
                                }),
                            );
                        }
                        Err(err) => {
                            self.cancel_trip(now, trip, err.to_string(), None, ctx);
                        }
                    }
                } else {
                    self.cancel_trip(
                        now,
                        trip,
                        format!(
                            "UsingBike trip couldn't find a way to start biking from {}",
                            start
                        ),
                        None,
                        ctx,
                    );
                }
            }
            TripSpec::UsingTransit { start, stop1, .. } => {
                assert_eq!(
                    person.state,
                    match start.connection {
                        SidewalkPOI::Building(b) => PersonState::Inside(b),
                        SidewalkPOI::Border(i) => {
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                i,
                            ));
                            PersonState::OffMap
                        }
                        SidewalkPOI::SuddenlyAppear => {
                            // Unclear which end of the sidewalk this person should be associated
                            // with. For interactively spawned people, doesn't really matter.
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                ctx.map.get_l(start.sidewalk_pos.lane()).src_i,
                            ));
                            PersonState::OffMap
                        }
                        _ => unreachable!(),
                    }
                );
                person.state = PersonState::Trip(trip);

                let walk_to = SidewalkSpot::bus_stop(stop1, ctx.map);
                let req = PathRequest {
                    start: start.sidewalk_pos,
                    end: walk_to.sidewalk_pos,
                    constraints: PathConstraints::Pedestrian,
                };
                match ctx.map.pathfind(req) {
                    Ok(path) => {
                        ctx.scheduler.push(
                            now,
                            Command::SpawnPed(CreatePedestrian {
                                id: person.ped,
                                speed: person.ped_speed,
                                start,
                                goal: walk_to,
                                path,
                                trip,
                                person: person.id,
                            }),
                        );
                    }
                    Err(err) => {
                        self.cancel_trip(now, trip, err.to_string(), None, ctx);
                    }
                }
            }
        }
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }
}

// Transitions between different legs of a trip
impl TripManager {
    pub fn agent_starting_trip_leg(&mut self, agent: AgentID, t: TripID) {
        if let Some(other) = self.active_trip_mode.get(&agent) {
            panic!("{} is doing both {} and {}?", agent, t, other);
        }
        self.active_trip_mode.insert(agent, t);
    }

    pub fn car_reached_parking_spot(
        &mut self,
        now: Time,
        car: CarID,
        spot: ParkingSpot,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
    ) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        match trip.legs.pop_front() {
            Some(TripLeg::Drive(c, DrivingGoal::ParkNear(_))) => {
                assert_eq!(car, c);
            }
            _ => unreachable!(),
        };

        match &trip.legs[0] {
            TripLeg::Walk(to) => match (spot, &to.connection) {
                (ParkingSpot::Offstreet(b1, _), SidewalkPOI::Building(b2)) if b1 == *b2 => {
                    assert_eq!(trip.legs.len(), 1);
                    trip.legs.pop_front().unwrap();

                    self.people[trip.person.0].state = PersonState::Inside(b1);
                    self.events
                        .push(Event::PersonEntersBuilding(trip.person, b1));
                    let id = trip.id;
                    self.trip_finished(now, id, ctx);
                    return;
                }
                _ => {}
            },
            _ => unreachable!(),
        };

        let id = trip.id;
        self.spawn_ped(
            now,
            id,
            SidewalkSpot::parking_spot(spot, ctx.map, ctx.parking),
            ctx,
        );
    }

    pub fn ped_reached_parking_spot(
        &mut self,
        now: Time,
        ped: PedestrianID,
        spot: ParkingSpot,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
    ) {
        self.events.push(Event::PedReachedParkingSpot(ped, spot));
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        trip.assert_walking_leg(SidewalkSpot::deferred_parking_spot());
        let parked_car = ctx.parking.get_car_at_spot(spot).unwrap().clone();
        let drive_to = match trip.legs[0] {
            TripLeg::Drive(c, ref to) => {
                assert_eq!(c, parked_car.vehicle.id);
                to.clone()
            }
            _ => unreachable!(),
        };

        let mut start =
            ctx.parking
                .spot_to_driving_pos(parked_car.spot, &parked_car.vehicle, ctx.map);
        match spot {
            ParkingSpot::Onstreet(_, _) => {}
            ParkingSpot::Offstreet(b, _) => {
                self.events
                    .push(Event::PersonEntersBuilding(trip.person, b));
                // Actually, to unpark, the car's front should be where it'll wind up at the end.
                start = Position::new(start.lane(), start.dist_along() + parked_car.vehicle.length);
            }
            ParkingSpot::Lot(_, _) => {
                start = Position::new(start.lane(), start.dist_along() + parked_car.vehicle.length);
            }
        }
        let end = drive_to.goal_pos(PathConstraints::Car, ctx.map).unwrap();
        let req = PathRequest {
            start,
            end,
            constraints: PathConstraints::Car,
        };

        let person = trip.person;
        let trip = trip.id;
        match self.maybe_spawn_car(ctx, now, trip, req, parked_car.vehicle.id) {
            Ok(path) => {
                let router = drive_to.make_router(parked_car.vehicle.id, path, ctx.map);
                ctx.scheduler.push(
                    now,
                    Command::SpawnCar(
                        CreateCar::for_parked_car(parked_car, router, trip, person),
                        true,
                    ),
                );
            }
            Err(err) => {
                // Move the car to the destination...
                ctx.parking.remove_parked_car(parked_car.clone());
                self.cancel_trip(now, trip, err.to_string(), Some(parked_car.vehicle), ctx);
            }
        }
    }

    pub fn ped_ready_to_bike(
        &mut self,
        now: Time,
        ped: PedestrianID,
        spot: SidewalkSpot,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        trip.assert_walking_leg(spot.clone());
        let (bike, drive_to) = match trip.legs[0] {
            TripLeg::Drive(bike, ref to) => (bike, to.clone()),
            _ => unreachable!(),
        };
        let driving_pos = match spot.connection {
            SidewalkPOI::BikeRack(p) => p,
            _ => unreachable!(),
        };

        let end = if let Some(end) = drive_to.goal_pos(PathConstraints::Bike, ctx.map) {
            end
        } else {
            let trip = trip.id;
            self.cancel_trip(
                now,
                trip,
                format!("no bike connection at {:?}", drive_to),
                None,
                ctx,
            );
            return;
        };
        let req = PathRequest {
            start: driving_pos,
            end,
            constraints: PathConstraints::Bike,
        };
        let maybe_router = if req.start.lane() == req.end.lane() {
            // TODO Convert to a walking trip! Ideally, do this earlier and convert the trip to
            // walking, like schedule_trip does
            Err(anyhow!(
                "biking to a different part of {} is silly, why not walk?",
                req.start.lane()
            ))
        } else {
            ctx.map
                .pathfind(req)
                .map(|path| drive_to.make_router(bike, path, ctx.map))
        };
        match maybe_router {
            Ok(router) => {
                ctx.scheduler.push(
                    now,
                    Command::SpawnCar(
                        CreateCar::for_appearing(
                            self.people[trip.person.0].get_vehicle(bike),
                            router,
                            trip.id,
                            trip.person,
                        ),
                        true,
                    ),
                );
            }
            Err(err) => {
                let trip = trip.id;
                self.cancel_trip(now, trip, err.to_string(), None, ctx);
            }
        }
    }

    pub fn bike_reached_end(
        &mut self,
        now: Time,
        bike: CarID,
        bike_rack: SidewalkSpot,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
    ) {
        self.events.push(Event::BikeStoppedAtSidewalk(
            bike,
            bike_rack.sidewalk_pos.lane(),
        ));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(bike)).unwrap().0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        match trip.legs.pop_front() {
            Some(TripLeg::Drive(c, DrivingGoal::ParkNear(_))) => {
                assert_eq!(c, bike);
            }
            _ => unreachable!(),
        };

        let id = trip.id;
        self.spawn_ped(now, id, bike_rack, ctx);
    }

    pub fn ped_reached_building(
        &mut self,
        now: Time,
        ped: PedestrianID,
        bldg: BuildingID,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        trip.assert_walking_leg(SidewalkSpot::building(bldg, ctx.map));

        self.people[trip.person.0].state = PersonState::Inside(bldg);
        self.events
            .push(Event::PersonEntersBuilding(trip.person, bldg));

        let id = trip.id;
        self.trip_finished(now, id, ctx);
    }

    /// If no route is returned, the pedestrian boarded a bus immediately.
    pub fn ped_reached_bus_stop(
        &mut self,
        now: Time,
        ped: PedestrianID,
        stop: BusStopID,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
        transit: &mut TransitSimState,
    ) -> Option<BusRouteID> {
        let trip = &mut self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        match trip.legs[0] {
            TripLeg::Walk(ref spot) => {
                assert_eq!(*spot, SidewalkSpot::bus_stop(stop, ctx.map));
            }
            _ => unreachable!(),
        }
        match trip.legs[1] {
            TripLeg::RideBus(route, maybe_stop2) => {
                self.events.push(Event::TripPhaseStarting(
                    trip.id,
                    trip.person,
                    None,
                    TripPhaseType::WaitingForBus(route, stop),
                ));
                if let Some(bus) = transit.ped_waiting_for_bus(
                    now,
                    ped,
                    trip.id,
                    trip.person,
                    stop,
                    route,
                    maybe_stop2,
                    ctx.map,
                ) {
                    trip.legs.pop_front();
                    self.active_trip_mode
                        .remove(&AgentID::Pedestrian(ped))
                        .unwrap();
                    self.active_trip_mode
                        .insert(AgentID::BusPassenger(trip.person, bus), trip.id);
                    self.people[trip.person.0].on_bus = Some(bus);
                    None
                } else {
                    Some(route)
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn ped_boarded_bus(
        &mut self,
        now: Time,
        ped: PedestrianID,
        bus: CarID,
        blocked_time: Duration,
        walking: &mut WalkingSimState,
    ) -> (TripID, PersonID) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;
        // No distance crossed between waiting for a bus and boarding

        trip.legs.pop_front();
        walking.ped_boarded_bus(now, ped);
        self.active_trip_mode
            .insert(AgentID::BusPassenger(trip.person, bus), trip.id);
        self.people[trip.person.0].on_bus = Some(bus);
        (trip.id, trip.person)
    }

    // TODO Need to characterize delay the bus experienced
    pub fn person_left_bus(&mut self, now: Time, person: PersonID, bus: CarID, ctx: &mut Ctx) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::BusPassenger(person, bus))
            .unwrap()
            .0];
        let start = match trip.legs.pop_front().unwrap() {
            TripLeg::RideBus(_, maybe_stop2) => SidewalkSpot::bus_stop(
                maybe_stop2.expect("someone left a bus, even though they should've ridden off-map"),
                ctx.map,
            ),
            _ => unreachable!(),
        };
        self.people[person.0].on_bus.take().unwrap();

        let id = trip.id;
        self.spawn_ped(now, id, start, ctx);
    }

    pub fn ped_reached_border(
        &mut self,
        now: Time,
        ped: PedestrianID,
        i: IntersectionID,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        match trip.legs.pop_front() {
            Some(TripLeg::Walk(spot)) => match spot.connection {
                SidewalkPOI::Border(i2) => assert_eq!(i, i2),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }

        if let TripEndpoint::Border(_) = trip.info.end {
            self.events.push(Event::PersonLeavesMap(
                trip.person,
                Some(AgentID::Pedestrian(ped)),
                i,
            ));
        }
        self.people[trip.person.0].state = PersonState::OffMap;

        let id = trip.id;
        self.trip_finished(now, id, ctx);
    }

    pub fn transit_rider_reached_border(
        &mut self,
        now: Time,
        person: PersonID,
        bus: CarID,
        ctx: &mut Ctx,
    ) {
        let agent = AgentID::BusPassenger(person, bus);
        let trip = &mut self.trips[self.active_trip_mode.remove(&agent).unwrap().0];

        match trip.legs.pop_front() {
            Some(TripLeg::RideBus(_, maybe_spot2)) => assert!(maybe_spot2.is_none()),
            _ => unreachable!(),
        }

        if let TripEndpoint::Border(i) = trip.info.end {
            self.events
                .push(Event::PersonLeavesMap(trip.person, Some(agent), i));
        } else {
            unreachable!()
        }
        self.people[trip.person.0].state = PersonState::OffMap;

        let id = trip.id;
        self.trip_finished(now, id, ctx);
    }

    pub fn car_or_bike_reached_border(
        &mut self,
        now: Time,
        car: CarID,
        i: IntersectionID,
        blocked_time: Duration,
        distance_crossed: Distance,
        ctx: &mut Ctx,
    ) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        trip.total_blocked_time += blocked_time;
        trip.total_distance += distance_crossed;

        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(c, DrivingGoal::Border(int, _)) => {
                assert_eq!(car, c);
                assert_eq!(i, int);
            }
            _ => unreachable!(),
        };

        self.people[trip.person.0].state = PersonState::OffMap;
        if let TripEndpoint::Border(_) = trip.info.end {
            self.events.push(Event::PersonLeavesMap(
                trip.person,
                Some(AgentID::Car(car)),
                i,
            ));
        }

        let id = trip.id;
        self.trip_finished(now, id, ctx);
    }

    fn trip_finished(&mut self, now: Time, id: TripID, ctx: &mut Ctx) {
        let trip = &mut self.trips[id.0];
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
        self.unfinished_trips -= 1;
        self.events.push(Event::TripFinished {
            trip: trip.id,
            mode: trip.info.mode,
            total_time: now - trip.info.departure,
            blocked_time: trip.total_blocked_time,
        });

        let person = trip.person;
        self.start_delayed_trip(now, person, ctx);
    }

    fn start_delayed_trip(&mut self, now: Time, id: PersonID, ctx: &mut Ctx) {
        let person = &mut self.people[id.0];
        if person.delayed_trips.is_empty() {
            return;
        }
        let (trip, spec) = person.delayed_trips.remove(0);
        if false {
            self.events.push(Event::Alert(
                AlertLocation::Person(person.id),
                format!(
                    "{} just freed up, so starting delayed trip {}",
                    person.id, trip
                ),
            ));
        }
        self.start_trip(now, trip, spec, ctx);
    }

    fn spawn_ped(&mut self, now: Time, id: TripID, start: SidewalkSpot, ctx: &mut Ctx) {
        let trip = &self.trips[id.0];
        let walk_to = match trip.legs[0] {
            TripLeg::Walk(ref to) => to.clone(),
            _ => unreachable!(),
        };

        let req = PathRequest {
            start: start.sidewalk_pos,
            end: walk_to.sidewalk_pos,
            constraints: PathConstraints::Pedestrian,
        };
        match ctx.map.pathfind(req) {
            Ok(path) => {
                let person = &self.people[trip.person.0];
                ctx.scheduler.push(
                    now,
                    Command::SpawnPed(CreatePedestrian {
                        id: person.ped,
                        speed: person.ped_speed,
                        start,
                        goal: walk_to,
                        path,
                        trip: id,
                        person: person.id,
                    }),
                );
            }
            Err(err) => {
                self.cancel_trip(now, id, err.to_string(), None, ctx);
            }
        }
    }

    /// Returns the path to use if successful. Caller is responsible for handling both the success
    /// and failure case.
    fn maybe_spawn_car(
        &mut self,
        ctx: &mut Ctx,
        now: Time,
        trip: TripID,
        req: PathRequest,
        car: CarID,
    ) -> Result<Path> {
        let path = ctx.map.pathfind(req)?;
        match ctx
            .cap
            .maybe_cap_path(path, now, car, ctx.intersections, ctx.map)
        {
            CapResult::OK(path) => Ok(path),
            CapResult::Reroute(path) => {
                self.trips[trip.0].info.capped = true;
                Ok(path)
            }
            CapResult::Cancel { reason } => {
                self.trips[trip.0].info.capped = true;
                bail!(reason)
            }
            CapResult::Delay(_) => todo!(),
        }
    }
}

// Cancelling trips
impl TripManager {
    /// Cancel a trip before it's started. The person will stay where they are.
    pub fn cancel_unstarted_trip(&mut self, id: TripID, reason: String) {
        let trip = &mut self.trips[id.0];
        self.unfinished_trips -= 1;
        trip.info.cancellation_reason = Some(reason);
        self.events
            .push(Event::TripCancelled(trip.id, trip.info.mode));
    }

    /// Cancel a trip after it's started. The person will be magically warped to their destination,
    /// along with their car, as if the trip had completed normally.
    pub fn cancel_trip(
        &mut self,
        now: Time,
        id: TripID,
        reason: String,
        abandoned_vehicle: Option<Vehicle>,
        ctx: &mut Ctx,
    ) {
        let trip = &mut self.trips[id.0];
        self.unfinished_trips -= 1;
        trip.info.cancellation_reason = Some(reason.to_string());
        self.events
            .push(Event::TripCancelled(trip.id, trip.info.mode));
        let person = trip.person;

        // Maintain consistentency for anyone listening to events
        if let PersonState::Inside(b) = self.people[person.0].state {
            self.events.push(Event::PersonLeavesBuilding(person, b));
        }
        // Warp to the destination
        self.people[person.0].state = match trip.info.end {
            TripEndpoint::Bldg(b) => {
                self.events.push(Event::PersonEntersBuilding(person, b));
                PersonState::Inside(b)
            }
            TripEndpoint::Border(i) => {
                self.events.push(Event::PersonLeavesMap(person, None, i));
                PersonState::OffMap
            }
            // Can't end trips here yet
            TripEndpoint::SuddenlyAppear(_) => unreachable!(),
        };

        // Don't forget the car!
        if let Some(vehicle) = abandoned_vehicle {
            if vehicle.vehicle_type == VehicleType::Car {
                if let TripEndpoint::Bldg(b) = trip.info.end {
                    let driving_lane = ctx.map.find_driving_lane_near_building(b);
                    if let Some(spot) = ctx
                        .parking
                        .get_all_free_spots(Position::start(driving_lane), &vehicle, b, ctx.map)
                        // TODO Could pick something closer, but meh, cancelled trips are bugs
                        // anyway
                        .get(0)
                        .map(|(spot, _)| spot.clone())
                        .or_else(|| {
                            ctx.parking
                                .path_to_free_parking_spot(driving_lane, &vehicle, b, ctx.map)
                                .map(|(_, spot, _)| spot)
                        })
                    {
                        self.events.push(Event::Alert(
                            AlertLocation::Person(person),
                            format!(
                                "{} had a trip cancelled, and their car was warped to {:?}",
                                person, spot
                            ),
                        ));
                        ctx.parking.reserve_spot(spot, vehicle.id);
                        ctx.parking.add_parked_car(ParkedCar {
                            vehicle,
                            spot,
                            parked_since: now,
                        });
                    } else {
                        self.events.push(Event::Alert(
                            AlertLocation::Person(person),
                            format!(
                                "{} had a trip cancelled, but nowhere to warp their car! Sucks.",
                                person
                            ),
                        ));
                    }
                }
            }
        } else {
            // If the trip was cancelled because we'e totally out of parking, don't forget to clean
            // this up.
            if let TripLeg::Drive(c, _) = &trip.legs[0] {
                if let Some(t) = self.active_trip_mode.remove(&AgentID::Car(*c)) {
                    assert_eq!(t, trip.id);
                }
            }
        }

        self.start_delayed_trip(now, person, ctx);
    }

    pub fn trip_abruptly_cancelled(&mut self, trip: TripID, agent: AgentID) {
        assert_eq!(self.active_trip_mode.remove(&agent), Some(trip));
    }
}

// Queries
impl TripManager {
    pub fn active_agents(&self) -> Vec<AgentID> {
        self.active_trip_mode.keys().cloned().collect()
    }
    pub fn active_agents_and_trips(&self) -> &BTreeMap<AgentID, TripID> {
        &self.active_trip_mode
    }
    pub fn num_active_agents(&self) -> usize {
        self.active_trip_mode.len()
    }

    pub fn trip_to_agent(&self, id: TripID) -> TripResult<AgentID> {
        if id.0 >= self.trips.len() {
            return TripResult::TripDoesntExist;
        }
        let trip = &self.trips[id.0];

        if trip.finished_at.is_some() {
            return TripResult::TripDone;
        }
        if trip.info.cancellation_reason.is_some() {
            return TripResult::TripCancelled;
        }
        if !trip.started {
            return TripResult::TripNotStarted;
        }

        let person = &self.people[trip.person.0];
        let a = match &trip.legs[0] {
            TripLeg::Walk(_) => AgentID::Pedestrian(person.ped),
            TripLeg::Drive(c, _) => AgentID::Car(*c),
            TripLeg::RideBus(_, _) => AgentID::BusPassenger(person.id, person.on_bus.unwrap()),
        };
        if self.active_trip_mode.get(&a) == Some(&id) {
            TripResult::Ok(a)
        } else {
            //panic!("{} should be ongoing, but no agent in active_trip_mode", id);
            TripResult::ModeChange
        }
    }

    /// This will be None for parked cars and buses. Should always work for pedestrians.
    pub fn agent_to_trip(&self, id: AgentID) -> Option<TripID> {
        self.active_trip_mode.get(&id).cloned()
    }

    pub fn debug_trip(&self, id: AgentID) {
        if let Some(t) = self.active_trip_mode.get(&id) {
            let trip = &self.trips[t.0];
            println!("{} has goal {:?}", trip.id, trip.legs.back().unwrap());
        } else {
            println!("{} has no trip, must be parked car", id);
        }
    }

    pub fn num_trips(&self) -> (usize, usize) {
        (
            self.trips.len() - self.unfinished_trips,
            self.unfinished_trips,
        )
    }
    pub fn num_agents(&self, transit: &TransitSimState) -> Counter<AgentType> {
        let mut cnt = Counter::new();
        for a in self.active_trip_mode.keys() {
            cnt.inc(a.to_type());
        }
        let (buses, trains) = transit.active_vehicles();
        cnt.add(AgentType::Bus, buses);
        cnt.add(AgentType::Train, trains);
        cnt
    }
    pub fn num_commuters_vehicles(
        &self,
        transit: &TransitSimState,
        walking: &WalkingSimState,
    ) -> CommutersVehiclesCounts {
        let (buses, trains) = transit.active_vehicles();
        let mut cnt = CommutersVehiclesCounts {
            walking_commuters: 0,
            walking_to_from_transit: 0,
            walking_to_from_car: 0,
            walking_to_from_bike: 0,

            cyclists: 0,

            sov_drivers: 0,

            buses,
            trains,
            bus_riders: 0,
            train_riders: 0,
        };

        for a in self.active_trip_mode.keys() {
            match a {
                AgentID::Car(c) => match c.1 {
                    VehicleType::Car => {
                        cnt.sov_drivers += 1;
                    }
                    VehicleType::Bike => {
                        cnt.cyclists += 1;
                    }
                    VehicleType::Bus | VehicleType::Train => unreachable!(),
                },
                AgentID::BusPassenger(_, c) => match c.1 {
                    VehicleType::Bus => {
                        cnt.bus_riders += 1;
                    }
                    VehicleType::Train => {
                        cnt.train_riders += 1;
                    }
                    VehicleType::Car | VehicleType::Bike => unreachable!(),
                },
                // These're counted separately
                AgentID::Pedestrian(_) => {}
            }
        }
        walking.populate_commuter_counts(&mut cnt);

        cnt
    }
    pub fn num_ppl(&self) -> (usize, usize, usize) {
        let mut ppl_in_bldg = 0;
        let mut ppl_off_map = 0;
        for p in &self.people {
            match p.state {
                PersonState::Trip(_) => {}
                PersonState::Inside(_) => {
                    ppl_in_bldg += 1;
                }
                PersonState::OffMap => {
                    ppl_off_map += 1;
                }
            }
        }
        (self.people.len(), ppl_in_bldg, ppl_off_map)
    }

    pub fn is_done(&self) -> bool {
        self.unfinished_trips == 0
    }

    pub fn trip_info(&self, id: TripID) -> TripInfo {
        self.trips[id.0].info.clone()
    }
    pub fn all_trip_info(&self) -> Vec<(TripID, TripInfo)> {
        self.trips.iter().map(|t| (t.id, t.info.clone())).collect()
    }
    pub fn finished_trip_details(&self, id: TripID) -> Option<(Duration, Duration, Distance)> {
        let t = &self.trips[id.0];
        Some((
            t.finished_at? - t.info.departure,
            t.total_blocked_time,
            t.total_distance,
        ))
    }
    pub fn trip_blocked_time(&self, id: TripID) -> Duration {
        let t = &self.trips[id.0];
        t.total_blocked_time
    }
    pub fn bldg_to_people(&self, b: BuildingID) -> Vec<PersonID> {
        let mut people = Vec::new();
        for p in &self.people {
            if p.state == PersonState::Inside(b) {
                people.push(p.id);
            }
        }
        people
    }

    pub fn get_person(&self, p: PersonID) -> Option<&Person> {
        self.people.get(p.0)
    }
    pub fn get_all_people(&self) -> &Vec<Person> {
        &self.people
    }

    pub fn trip_to_person(&self, id: TripID) -> Option<PersonID> {
        Some(self.trips.get(id.0)?.person)
    }

    pub fn all_arrivals_at_border(&self, at: IntersectionID) -> Vec<(Time, AgentType)> {
        let mut times = Vec::new();
        for t in &self.trips {
            if t.info.cancellation_reason.is_some() {
                continue;
            }
            if let TripEndpoint::Border(i) = t.info.start {
                if i == at {
                    // We can make some assumptions here.
                    let agent_type = match t.info.mode {
                        TripMode::Walk => AgentType::Pedestrian,
                        TripMode::Bike => AgentType::Bike,
                        TripMode::Drive => AgentType::Car,
                        // TODO Not true for long. People will be able to spawn at borders already
                        // on a bus.
                        TripMode::Transit => AgentType::Pedestrian,
                    };
                    times.push((t.info.departure, agent_type));
                }
            }
        }
        times.sort();
        times
    }

    /// Recreate the Scenario from an instantiated simulation. The results should match the
    /// original Scenario used.
    pub fn generate_scenario(&self, map: &Map, name: String) -> Scenario {
        let mut scenario = Scenario::empty(map, &name);
        for p in &self.people {
            scenario.people.push(PersonSpec {
                orig_id: p.orig_id,
                origin: self.trips[p.trips[0].0].info.start.clone(),
                trips: p
                    .trips
                    .iter()
                    .map(|t| {
                        let trip = &self.trips[t.0];
                        IndividTrip::new(
                            trip.info.departure,
                            trip.info.purpose,
                            trip.info.end.clone(),
                            trip.info.mode,
                        )
                    })
                    .collect(),
            });
        }
        scenario
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Trip {
    id: TripID,
    info: TripInfo,
    started: bool,
    finished_at: Option<Time>,
    total_blocked_time: Duration,
    total_distance: Distance,
    legs: VecDeque<TripLeg>,
    person: PersonID,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TripInfo {
    /// Scheduled departure; the start may be delayed if the previous trip is taking too long.
    pub departure: Time,
    pub mode: TripMode,
    pub start: TripEndpoint,
    pub end: TripEndpoint,
    pub purpose: TripPurpose,
    /// Did a ScenarioModifier apply to this?
    pub modified: bool,
    /// Was this trip affected by a congestion cap?
    pub capped: bool,
    pub cancellation_reason: Option<String>,
}

impl Trip {
    fn assert_walking_leg(&mut self, goal: SidewalkSpot) {
        match self.legs.pop_front() {
            Some(TripLeg::Walk(spot)) => {
                assert_eq!(goal, spot);
            }
            _ => unreachable!(),
        }
    }
}

/// These don't specify where the leg starts, since it might be unknown -- like when we drive and
/// don't know where we'll wind up parking.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub(crate) enum TripLeg {
    Walk(SidewalkSpot),
    /// A person may own many vehicles, so specify which they use
    Drive(CarID, DrivingGoal),
    /// Maybe get off at a stop, maybe ride off-map
    RideBus(BusRouteID, Option<BusStopID>),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum TripMode {
    Walk,
    Bike,
    Transit,
    Drive,
}

impl TripMode {
    pub fn all() -> Vec<TripMode> {
        vec![
            TripMode::Walk,
            TripMode::Bike,
            TripMode::Transit,
            TripMode::Drive,
        ]
    }

    pub fn verb(self) -> &'static str {
        match self {
            TripMode::Walk => "walk",
            TripMode::Bike => "bike",
            TripMode::Transit => "use transit",
            TripMode::Drive => "drive",
        }
    }

    // If I used "present participle" in a method name, I'd never live it down.
    pub fn ongoing_verb(self) -> &'static str {
        match self {
            TripMode::Walk => "walking",
            TripMode::Bike => "biking",
            TripMode::Transit => "using transit",
            TripMode::Drive => "driving",
        }
    }

    pub fn noun(self) -> &'static str {
        match self {
            TripMode::Walk => "Pedestrian",
            TripMode::Bike => "Bike",
            TripMode::Transit => "Bus",
            TripMode::Drive => "Car",
        }
    }

    pub fn to_constraints(self) -> PathConstraints {
        match self {
            TripMode::Walk => PathConstraints::Pedestrian,
            TripMode::Bike => PathConstraints::Bike,
            // TODO WRONG
            TripMode::Transit => PathConstraints::Bus,
            TripMode::Drive => PathConstraints::Car,
        }
    }

    pub fn from_constraints(c: PathConstraints) -> TripMode {
        match c {
            PathConstraints::Pedestrian => TripMode::Walk,
            PathConstraints::Bike => TripMode::Bike,
            // TODO The bijection breaks down... transit rider vs train vs bus...
            PathConstraints::Bus | PathConstraints::Train => TripMode::Transit,
            PathConstraints::Car => TripMode::Drive,
        }
    }
}

pub enum TripResult<T> {
    Ok(T),
    ModeChange,
    TripDone,
    TripDoesntExist,
    TripNotStarted,
    TripCancelled,
}

impl<T> TripResult<T> {
    pub fn ok(self) -> Option<T> {
        match self {
            TripResult::Ok(data) => Some(data),
            _ => None,
        }
    }

    pub fn propagate_error<X>(self) -> TripResult<X> {
        match self {
            TripResult::Ok(_) => panic!("TripResult is Ok, can't propagate_error"),
            TripResult::ModeChange => TripResult::ModeChange,
            TripResult::TripDone => TripResult::TripDone,
            TripResult::TripDoesntExist => TripResult::TripDoesntExist,
            TripResult::TripNotStarted => TripResult::TripNotStarted,
            TripResult::TripCancelled => TripResult::TripCancelled,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Person {
    pub id: PersonID,
    pub orig_id: Option<OrigPersonID>,
    pub trips: Vec<TripID>,
    // TODO home
    pub state: PersonState,

    pub ped: PedestrianID,
    pub ped_speed: Speed,
    /// Both cars and bikes
    pub vehicles: Vec<Vehicle>,

    delayed_trips: Vec<(TripID, TripSpec)>,
    on_bus: Option<CarID>,
}

impl Person {
    fn get_vehicle(&self, id: CarID) -> Vehicle {
        self.vehicles.iter().find(|v| v.id == id).unwrap().clone()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum PersonState {
    Trip(TripID),
    Inside(BuildingID),
    OffMap,
}

/// The number of active vehicles and commuters, broken into different categories.
pub struct CommutersVehiclesCounts {
    pub walking_commuters: usize,
    pub walking_to_from_transit: usize,
    pub walking_to_from_car: usize,
    pub walking_to_from_bike: usize,

    pub cyclists: usize,

    pub sov_drivers: usize,

    pub buses: usize,
    pub trains: usize,
    pub bus_riders: usize,
    pub train_riders: usize,
}

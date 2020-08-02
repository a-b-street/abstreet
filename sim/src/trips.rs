use crate::{
    AgentID, AgentType, AlertLocation, CarID, Command, CreateCar, CreatePedestrian, DrivingGoal,
    Event, OffMapLocation, OrigPersonID, ParkedCar, ParkingSimState, ParkingSpot, PedestrianID,
    PersonID, Scheduler, SidewalkPOI, SidewalkSpot, TransitSimState, TripID, TripPhaseType,
    TripSpec, Vehicle, VehicleSpec, VehicleType, WalkingSimState,
};
use abstutil::{deserialize_btreemap, serialize_btreemap, Counter};
use geom::{Duration, Speed, Time};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, Map, Path, PathConstraints, PathRequest,
    Position,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct TripManager {
    trips: Vec<Trip>,
    people: Vec<Person>,
    // For quick lookup of active agents
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    active_trip_mode: BTreeMap<AgentID, TripID>,
    unfinished_trips: usize,
    pub pathfinding_upfront: bool,

    car_id_counter: usize,

    events: Vec<Event>,
}

impl TripManager {
    pub fn new(pathfinding_upfront: bool) -> TripManager {
        TripManager {
            trips: Vec::new(),
            people: Vec::new(),
            active_trip_mode: BTreeMap::new(),
            unfinished_trips: 0,
            car_id_counter: 0,
            events: Vec::new(),
            pathfinding_upfront,
        }
    }

    // TODO assert the specs are correct yo
    pub fn new_person(
        &mut self,
        id: PersonID,
        orig_id: Option<OrigPersonID>,
        ped_speed: Speed,
        vehicle_specs: Vec<VehicleSpec>,
    ) {
        assert_eq!(id.0, self.people.len());
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
    }
    pub fn random_person(&mut self, ped_speed: Speed, vehicle_specs: Vec<VehicleSpec>) -> &Person {
        let id = PersonID(self.people.len());
        self.new_person(id, None, ped_speed, vehicle_specs);
        self.get_person(id).unwrap()
    }

    pub fn new_car_id(&mut self) -> usize {
        let id = self.car_id_counter;
        self.car_id_counter += 1;
        id
    }

    pub fn new_trip(
        &mut self,
        person: PersonID,
        departure: Time,
        start: TripEndpoint,
        mode: TripMode,
        modified: bool,
        legs: Vec<TripLeg>,
        map: &Map,
    ) -> TripID {
        assert!(!legs.is_empty());
        // TODO Make sure the legs constitute a valid state machine.

        let id = TripID(self.trips.len());
        let end = match legs.last() {
            Some(TripLeg::Walk(ref spot)) => match spot.connection {
                SidewalkPOI::Building(b) => TripEndpoint::Bldg(b),
                SidewalkPOI::Border(i, ref loc) => TripEndpoint::Border(i, loc.clone()),
                _ => unreachable!(),
            },
            Some(TripLeg::Drive(_, ref goal)) => match goal {
                DrivingGoal::ParkNear(b) => TripEndpoint::Bldg(*b),
                DrivingGoal::Border(i, _, loc) => TripEndpoint::Border(*i, loc.clone()),
            },
            Some(TripLeg::Remote(ref to)) => {
                TripEndpoint::Border(map.all_incoming_borders()[0].id, Some(to.clone()))
            }
            Some(TripLeg::RideBus(r, ref maybe_stop2)) => {
                assert!(maybe_stop2.is_none());
                // TODO No way to plumb OffMapLocation here
                TripEndpoint::Border(map.get_l(map.get_br(*r).end_border.unwrap()).dst_i, None)
            }
            _ => unreachable!(),
        };
        let trip = Trip {
            id,
            info: TripInfo {
                departure,
                mode,
                start,
                end,
                modified,
            },
            person,
            started: false,
            finished_at: None,
            total_blocked_time: Duration::ZERO,
            aborted: false,
            cancelled: false,
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
                TripEndpoint::Border(_, ref loc) => {
                    if let Some(loc) = loc {
                        self.events
                            .push(Event::PersonEntersRemoteBuilding(trip.person, loc.clone()));
                    }
                    PersonState::OffMap
                }
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
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        trip.total_blocked_time += blocked_time;

        match trip.legs.pop_front() {
            Some(TripLeg::Drive(c, DrivingGoal::ParkNear(_))) => {
                assert_eq!(car, c);
            }
            _ => unreachable!(),
        };

        match &trip.legs[0] {
            TripLeg::Walk(to) => match (spot, &to.connection) {
                (ParkingSpot::Offstreet(b1, _), SidewalkPOI::Building(b2)) if b1 == *b2 => {
                    // Do the relevant parts of ped_reached_parking_spot.
                    assert_eq!(trip.legs.len(), 1);
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
                    self.people[person.0].state = PersonState::Inside(b1);
                    self.events.push(Event::PersonEntersBuilding(person, b1));
                    self.person_finished_trip(now, person, parking, scheduler, map);
                    return;
                }
                _ => {}
            },
            _ => unreachable!(),
        };

        if !trip.spawn_ped(
            now,
            SidewalkSpot::parking_spot(spot, map, parking),
            &self.people[trip.person.0],
            map,
            scheduler,
            &mut self.events,
        ) {
            self.unfinished_trips -= 1;
        }
    }

    pub fn ped_reached_parking_spot(
        &mut self,
        now: Time,
        ped: PedestrianID,
        spot: ParkingSpot,
        blocked_time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        self.events.push(Event::PedReachedParkingSpot(ped, spot));
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        trip.assert_walking_leg(SidewalkSpot::deferred_parking_spot());
        let parked_car = parking.get_car_at_spot(spot).unwrap().clone();
        let drive_to = match trip.legs[0] {
            TripLeg::Drive(c, ref to) => {
                assert_eq!(c, parked_car.vehicle.id);
                to.clone()
            }
            _ => unreachable!(),
        };

        let mut start = parking.spot_to_driving_pos(parked_car.spot, &parked_car.vehicle, map);
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
        let end = drive_to.goal_pos(PathConstraints::Car, map).unwrap();
        let req = PathRequest {
            start,
            end,
            constraints: PathConstraints::Car,
        };
        let path = if let Some(p) = map.pathfind(req.clone()) {
            p
        } else {
            self.events.push(Event::Alert(
                AlertLocation::Person(trip.person),
                format!(
                    "Aborting {} because no path for the car portion! {} to {}",
                    trip.id, start, end
                ),
            ));
            // Move the car to the destination...
            parking.remove_parked_car(parked_car.clone());
            let trip = trip.id;
            self.abort_trip(now, trip, Some(parked_car.vehicle), parking, scheduler, map);
            return;
        };

        let router = drive_to.make_router(parked_car.vehicle.id, path, map);
        scheduler.push(
            now,
            Command::SpawnCar(
                CreateCar::for_parked_car(
                    parked_car,
                    router,
                    req,
                    start.dist_along(),
                    trip.id,
                    trip.person,
                ),
                true,
            ),
        );
    }

    pub fn ped_ready_to_bike(
        &mut self,
        now: Time,
        ped: PedestrianID,
        spot: SidewalkSpot,
        blocked_time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        trip.assert_walking_leg(spot.clone());
        let (bike, drive_to) = match trip.legs[0] {
            TripLeg::Drive(bike, ref to) => (bike, to.clone()),
            _ => unreachable!(),
        };
        let driving_pos = match spot.connection {
            SidewalkPOI::BikeRack(p) => p,
            _ => unreachable!(),
        };

        let end = if let Some(end) = drive_to.goal_pos(PathConstraints::Bike, map) {
            end
        } else {
            self.events.push(Event::Alert(
                AlertLocation::Person(trip.person),
                format!(
                    "Aborting {} because no bike connection at {:?}",
                    trip.id, drive_to
                ),
            ));
            let trip = trip.id;
            self.abort_trip(now, trip, None, parking, scheduler, map);
            return;
        };
        let req = PathRequest {
            start: driving_pos,
            end,
            constraints: PathConstraints::Bike,
        };
        if let Some(router) = map
            .pathfind(req.clone())
            .map(|path| drive_to.make_router(bike, path, map))
        {
            scheduler.push(
                now,
                Command::SpawnCar(
                    CreateCar::for_appearing(
                        self.people[trip.person.0].get_vehicle(bike),
                        driving_pos,
                        router,
                        req,
                        trip.id,
                        trip.person,
                    ),
                    true,
                ),
            );
        } else {
            self.events.push(Event::Alert(
                AlertLocation::Person(trip.person),
                format!(
                    "Aborting {} because no path for the bike portion (or sidewalk connection at \
                     the end)! {} to {}",
                    trip.id, driving_pos, end
                ),
            ));
            let trip = trip.id;
            self.abort_trip(now, trip, None, parking, scheduler, map);
        }
    }

    pub fn bike_reached_end(
        &mut self,
        now: Time,
        bike: CarID,
        bike_rack: SidewalkSpot,
        blocked_time: Duration,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        self.events.push(Event::BikeStoppedAtSidewalk(
            bike,
            bike_rack.sidewalk_pos.lane(),
        ));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(bike)).unwrap().0];
        trip.total_blocked_time += blocked_time;

        match trip.legs.pop_front() {
            Some(TripLeg::Drive(c, DrivingGoal::ParkNear(_))) => {
                assert_eq!(c, bike);
            }
            _ => unreachable!(),
        };

        if !trip.spawn_ped(
            now,
            bike_rack,
            &self.people[trip.person.0],
            map,
            scheduler,
            &mut self.events,
        ) {
            self.unfinished_trips -= 1;
        }
    }

    pub fn ped_reached_building(
        &mut self,
        now: Time,
        ped: PedestrianID,
        bldg: BuildingID,
        blocked_time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        trip.assert_walking_leg(SidewalkSpot::building(bldg, map));
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
        self.people[person.0].state = PersonState::Inside(bldg);
        self.events.push(Event::PersonEntersBuilding(person, bldg));
        self.person_finished_trip(now, person, parking, scheduler, map);
    }

    // If no route is returned, the pedestrian boarded a bus immediately.
    pub fn ped_reached_bus_stop(
        &mut self,
        now: Time,
        ped: PedestrianID,
        stop: BusStopID,
        blocked_time: Duration,
        map: &Map,
        transit: &mut TransitSimState,
    ) -> Option<BusRouteID> {
        let trip = &mut self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];
        trip.total_blocked_time += blocked_time;

        match trip.legs[0] {
            TripLeg::Walk(ref spot) => {
                assert_eq!(*spot, SidewalkSpot::bus_stop(stop, map));
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
                    map,
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

        trip.legs.pop_front();
        walking.ped_boarded_bus(now, ped);
        self.active_trip_mode
            .insert(AgentID::BusPassenger(trip.person, bus), trip.id);
        self.people[trip.person.0].on_bus = Some(bus);
        (trip.id, trip.person)
    }

    // TODO Need to characterize delay the bus experienced
    pub fn person_left_bus(
        &mut self,
        now: Time,
        person: PersonID,
        bus: CarID,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::BusPassenger(person, bus))
            .unwrap()
            .0];
        let start = match trip.legs.pop_front().unwrap() {
            TripLeg::RideBus(_, maybe_stop2) => SidewalkSpot::bus_stop(
                maybe_stop2.expect("someone left a bus, even though they should've ridden off-map"),
                map,
            ),
            _ => unreachable!(),
        };
        self.people[person.0].on_bus.take().unwrap();

        if !trip.spawn_ped(
            now,
            start,
            &self.people[trip.person.0],
            map,
            scheduler,
            &mut self.events,
        ) {
            self.unfinished_trips -= 1;
        }
    }

    pub fn ped_reached_border(
        &mut self,
        now: Time,
        ped: PedestrianID,
        i: IntersectionID,
        blocked_time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        match trip.legs.pop_front() {
            Some(TripLeg::Walk(spot)) => match spot.connection {
                SidewalkPOI::Border(i2, _) => assert_eq!(i, i2),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
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
        if let TripEndpoint::Border(_, ref loc) = trip.info.end {
            self.events.push(Event::PersonLeavesMap(
                person,
                Some(AgentID::Pedestrian(ped)),
                i,
                loc.clone(),
            ));
        }
        self.people[person.0].state = PersonState::OffMap;
        self.person_finished_trip(now, person, parking, scheduler, map);
    }

    pub fn transit_rider_reached_border(
        &mut self,
        now: Time,
        person: PersonID,
        bus: CarID,
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let agent = AgentID::BusPassenger(person, bus);
        let trip = &mut self.trips[self.active_trip_mode.remove(&agent).unwrap().0];

        match trip.legs.pop_front() {
            Some(TripLeg::RideBus(_, maybe_spot2)) => assert!(maybe_spot2.is_none()),
            _ => unreachable!(),
        }
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
        if let TripEndpoint::Border(i, ref loc) = trip.info.end {
            self.events
                .push(Event::PersonLeavesMap(person, Some(agent), i, loc.clone()));
        } else {
            unreachable!()
        }
        self.people[person.0].state = PersonState::OffMap;
        self.person_finished_trip(now, person, parking, scheduler, map);
    }

    pub fn car_or_bike_reached_border(
        &mut self,
        now: Time,
        car: CarID,
        i: IntersectionID,
        blocked_time: Duration,
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        trip.total_blocked_time += blocked_time;

        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(c, DrivingGoal::Border(int, _, _)) => {
                assert_eq!(car, c);
                assert_eq!(i, int);
            }
            _ => unreachable!(),
        };
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
        self.people[person.0].state = PersonState::OffMap;
        if let TripEndpoint::Border(_, ref loc) = trip.info.end {
            self.events.push(Event::PersonLeavesMap(
                person,
                Some(AgentID::Car(car)),
                i,
                loc.clone(),
            ));
        }
        self.person_finished_trip(now, person, parking, scheduler, map);
    }

    pub fn remote_trip_finished(
        &mut self,
        now: Time,
        id: TripID,
        map: &Map,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[id.0];

        let to = match trip.legs.pop_front() {
            Some(TripLeg::Remote(to)) => to,
            _ => unreachable!(),
        };
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
        self.events
            .push(Event::PersonEntersRemoteBuilding(person, to));
        self.people[person.0].state = PersonState::OffMap;
        self.person_finished_trip(now, person, parking, scheduler, map);
    }

    // Different than aborting a trip. Don't warp any vehicles or change where the person is.
    pub fn cancel_trip(&mut self, id: TripID) {
        let trip = &mut self.trips[id.0];
        self.unfinished_trips -= 1;
        trip.cancelled = true;
        // TODO Still representing the same way in analytics
        self.events.push(Event::TripAborted(trip.id));
    }

    pub fn abort_trip(
        &mut self,
        now: Time,
        id: TripID,
        abandoned_vehicle: Option<Vehicle>,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
        map: &Map,
    ) {
        let trip = &mut self.trips[id.0];
        self.unfinished_trips -= 1;
        trip.aborted = true;
        self.events.push(Event::TripAborted(trip.id));
        let person = trip.person;

        // Maintain consistentency for anyone listening to events
        if let PersonState::Inside(b) = self.people[person.0].state {
            self.events.push(Event::PersonLeavesBuilding(person, b));
        }
        match trip.info.end {
            TripEndpoint::Bldg(b) => {
                self.events.push(Event::PersonEntersBuilding(person, b));
            }
            TripEndpoint::Border(i, ref loc) => {
                self.events
                    .push(Event::PersonLeavesMap(person, None, i, loc.clone()));
            }
        }

        // Warp to the destination
        self.people[person.0].state = match trip.info.end {
            TripEndpoint::Bldg(b) => PersonState::Inside(b),
            TripEndpoint::Border(_, _) => PersonState::OffMap,
        };
        // Don't forget the car!
        if let Some(vehicle) = abandoned_vehicle {
            if vehicle.vehicle_type == VehicleType::Car {
                if let TripEndpoint::Bldg(b) = trip.info.end {
                    let driving_lane = map.find_driving_lane_near_building(b);
                    if let Some(spot) = parking
                        .get_all_free_spots(Position::start(driving_lane), &vehicle, b, map)
                        // TODO Could pick something closer, but meh, aborted trips are bugs anyway
                        .get(0)
                        .map(|(spot, _)| spot.clone())
                        .or_else(|| {
                            parking
                                .path_to_free_parking_spot(driving_lane, &vehicle, b, map)
                                .map(|(_, spot, _)| spot)
                        })
                    {
                        self.events.push(Event::Alert(
                            AlertLocation::Person(person),
                            format!(
                                "{} had a trip aborted, and their car was warped to {:?}",
                                person, spot
                            ),
                        ));
                        parking.reserve_spot(spot);
                        parking.add_parked_car(ParkedCar { vehicle, spot });
                    } else {
                        self.events.push(Event::Alert(
                            AlertLocation::Person(person),
                            format!(
                                "{} had a trip aborted, but nowhere to warp their car! Sucks.",
                                person
                            ),
                        ));
                    }
                }
            }
        } else {
            // If the trip was aborted because we'e totally out of parking, don't forget to clean
            // this up.
            if let TripLeg::Drive(c, _) = &trip.legs[0] {
                if let Some(t) = self.active_trip_mode.remove(&AgentID::Car(*c)) {
                    assert_eq!(t, trip.id);
                }
            }
        }

        self.person_finished_trip(now, person, parking, scheduler, map);
    }

    pub fn active_agents(&self) -> Vec<AgentID> {
        self.active_trip_mode.keys().cloned().collect()
    }

    pub fn get_active_trips(&self) -> Vec<TripID> {
        self.active_trip_mode.values().cloned().collect()
    }

    pub fn trip_to_agent(&self, id: TripID) -> TripResult<AgentID> {
        if id.0 >= self.trips.len() {
            return TripResult::TripDoesntExist;
        }
        let trip = &self.trips[id.0];

        if trip.finished_at.is_some() {
            return TripResult::TripDone;
        }
        if trip.aborted {
            return TripResult::TripAborted;
        }
        if trip.cancelled {
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
            TripLeg::Remote(_) => {
                return TripResult::RemoteTrip;
            }
        };
        if self.active_trip_mode.get(&a) == Some(&id) {
            TripResult::Ok(a)
        } else {
            //panic!("{} should be ongoing, but no agent in active_trip_mode", id);
            TripResult::ModeChange
        }
    }

    // This will be None for parked cars and buses. Should always work for pedestrians.
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

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    pub fn trip_info(&self, id: TripID) -> TripInfo {
        self.trips[id.0].info.clone()
    }
    pub fn all_trip_info(&self) -> Vec<(TripID, TripInfo)> {
        self.trips.iter().map(|t| (t.id, t.info.clone())).collect()
    }
    pub fn finished_trip_time(&self, id: TripID) -> Option<(Duration, Duration)> {
        let t = &self.trips[id.0];
        Some((t.finished_at? - t.info.departure, t.total_blocked_time))
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

    pub fn trip_to_person(&self, id: TripID) -> PersonID {
        self.trips[id.0].person
    }

    fn person_finished_trip(
        &mut self,
        now: Time,
        person: PersonID,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
        map: &Map,
    ) {
        let person = &mut self.people[person.0];
        if person.delayed_trips.is_empty() {
            return;
        }
        let (trip, spec, maybe_req, maybe_path) = person.delayed_trips.remove(0);
        if false {
            self.events.push(Event::Alert(
                AlertLocation::Person(person.id),
                format!(
                    "{} just freed up, so starting delayed trip {}",
                    person.id, trip
                ),
            ));
        }
        self.start_trip(
            now, trip, spec, maybe_req, maybe_path, parking, scheduler, map,
        );
    }

    pub fn start_trip(
        &mut self,
        now: Time,
        trip: TripID,
        spec: TripSpec,
        maybe_req: Option<PathRequest>,
        mut maybe_path: Option<Path>,
        parking: &mut ParkingSimState,
        scheduler: &mut Scheduler,
        map: &Map,
    ) {
        assert!(!self.trips[trip.0].cancelled);
        assert!(!self.trips[trip.0].aborted);
        if !self.pathfinding_upfront && maybe_path.is_none() && maybe_req.is_some() {
            maybe_path = map.pathfind(maybe_req.clone().unwrap());
        }

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
            person
                .delayed_trips
                .push((trip, spec, maybe_req, maybe_path));
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
                origin,
            } => {
                assert_eq!(person.state, PersonState::OffMap);
                self.events.push(Event::PersonEntersMap(
                    person.id,
                    AgentID::Car(use_vehicle),
                    map.get_l(start_pos.lane()).src_i,
                    origin,
                ));
                person.state = PersonState::Trip(trip);

                let vehicle = person.get_vehicle(use_vehicle);
                assert!(parking.lookup_parked_car(vehicle.id).is_none());
                let req = maybe_req.unwrap();
                if let Some(router) = maybe_path.map(|path| goal.make_router(vehicle.id, path, map))
                {
                    scheduler.push(
                        now,
                        Command::SpawnCar(
                            CreateCar::for_appearing(
                                vehicle, start_pos, router, req, trip, person.id,
                            ),
                            retry_if_no_room,
                        ),
                    );
                } else {
                    self.events.push(Event::Alert(
                        AlertLocation::Person(person.id),
                        format!(
                            "VehicleAppearing trip couldn't find the first path: {}",
                            req
                        ),
                    ));
                    self.abort_trip(now, trip, Some(vehicle), parking, scheduler, map);
                }
            }
            TripSpec::NoRoomToSpawn {
                i,
                use_vehicle,
                error,
                ..
            } => {
                self.events.push(Event::Alert(
                    AlertLocation::Intersection(i),
                    format!("{} couldn't spawn at border {}: {}", person.id, i, error),
                ));
                let vehicle = person.get_vehicle(use_vehicle);
                self.abort_trip(now, trip, Some(vehicle), parking, scheduler, map);
            }
            TripSpec::UsingParkedCar {
                car, start_bldg, ..
            } => {
                assert_eq!(person.state, PersonState::Inside(start_bldg));
                person.state = PersonState::Trip(trip);

                // TODO For now, use the car we decided to statically. That makes sense in most
                // cases.

                if let Some(parked_car) = parking.lookup_parked_car(car).cloned() {
                    let start = SidewalkSpot::building(start_bldg, map);
                    let walking_goal = SidewalkSpot::parking_spot(parked_car.spot, map, parking);
                    let req = PathRequest {
                        start: start.sidewalk_pos,
                        end: walking_goal.sidewalk_pos,
                        constraints: PathConstraints::Pedestrian,
                    };
                    if let Some(path) = map.pathfind(req.clone()) {
                        scheduler.push(
                            now,
                            Command::SpawnPed(CreatePedestrian {
                                id: person.ped,
                                speed: person.ped_speed,
                                start,
                                goal: walking_goal,
                                path,
                                req,
                                trip,
                                person: person.id,
                            }),
                        );
                    } else {
                        self.events.push(Event::Alert(
                            AlertLocation::Person(person.id),
                            format!("UsingParkedCar trip couldn't find the walking path {}", req),
                        ));
                        // Move the car to the destination
                        parking.remove_parked_car(parked_car.clone());
                        self.abort_trip(
                            now,
                            trip,
                            Some(parked_car.vehicle),
                            parking,
                            scheduler,
                            map,
                        );
                    }
                } else {
                    // This should only happen when a driving trip has been aborted and there was
                    // absolutely no room to warp the car.
                    self.events.push(Event::Alert(
                        AlertLocation::Person(person.id),
                        format!(
                            "{} should have {} parked somewhere, but it's unavailable, so \
                             aborting {}",
                            person.id, car, trip
                        ),
                    ));
                    self.abort_trip(now, trip, None, parking, scheduler, map);
                }
            }
            TripSpec::JustWalking { start, goal } => {
                assert_eq!(
                    person.state,
                    match start.connection {
                        SidewalkPOI::Building(b) => PersonState::Inside(b),
                        SidewalkPOI::Border(i, ref loc) => {
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                i,
                                loc.clone(),
                            ));
                            PersonState::OffMap
                        }
                        SidewalkPOI::SuddenlyAppear => {
                            // Unclear which end of the sidewalk this person should be associated
                            // with. For interactively spawned people, doesn't really matter.
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                map.get_l(start.sidewalk_pos.lane()).src_i,
                                None,
                            ));
                            PersonState::OffMap
                        }
                        _ => unreachable!(),
                    }
                );
                person.state = PersonState::Trip(trip);

                let req = maybe_req.unwrap();
                if let Some(path) = maybe_path {
                    scheduler.push(
                        now,
                        Command::SpawnPed(CreatePedestrian {
                            id: person.ped,
                            speed: person.ped_speed,
                            start,
                            goal,
                            path,
                            req,
                            trip,
                            person: person.id,
                        }),
                    );
                } else {
                    self.events.push(Event::Alert(
                        AlertLocation::Person(person.id),
                        format!("JustWalking trip couldn't find the first path {}", req),
                    ));
                    self.abort_trip(now, trip, None, parking, scheduler, map);
                }
            }
            TripSpec::UsingBike { start, .. } => {
                assert_eq!(person.state, PersonState::Inside(start));
                person.state = PersonState::Trip(trip);

                if let Some(walk_to) = SidewalkSpot::bike_rack(start, map) {
                    let req = maybe_req.unwrap();
                    if let Some(path) = maybe_path {
                        scheduler.push(
                            now,
                            Command::SpawnPed(CreatePedestrian {
                                id: person.ped,
                                speed: person.ped_speed,
                                start: SidewalkSpot::building(start, map),
                                goal: walk_to,
                                path,
                                req,
                                trip,
                                person: person.id,
                            }),
                        );
                    } else {
                        self.events.push(Event::Alert(
                            AlertLocation::Person(person.id),
                            format!("UsingBike trip couldn't find the first path {}", req),
                        ));
                        self.abort_trip(now, trip, None, parking, scheduler, map);
                    }
                } else {
                    self.events.push(Event::Alert(
                        AlertLocation::Person(person.id),
                        format!(
                            "UsingBike trip couldn't find a way to start biking from {}",
                            start
                        ),
                    ));
                    self.abort_trip(now, trip, None, parking, scheduler, map);
                }
            }
            TripSpec::UsingTransit { start, stop1, .. } => {
                assert_eq!(
                    person.state,
                    match start.connection {
                        SidewalkPOI::Building(b) => PersonState::Inside(b),
                        SidewalkPOI::Border(i, ref loc) => {
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                i,
                                loc.clone(),
                            ));
                            PersonState::OffMap
                        }
                        SidewalkPOI::SuddenlyAppear => {
                            // Unclear which end of the sidewalk this person should be associated
                            // with. For interactively spawned people, doesn't really matter.
                            self.events.push(Event::PersonEntersMap(
                                person.id,
                                AgentID::Pedestrian(person.ped),
                                map.get_l(start.sidewalk_pos.lane()).src_i,
                                None,
                            ));
                            PersonState::OffMap
                        }
                        _ => unreachable!(),
                    }
                );
                person.state = PersonState::Trip(trip);

                let walk_to = SidewalkSpot::bus_stop(stop1, map);
                let req = maybe_req.unwrap();
                if let Some(path) = maybe_path {
                    scheduler.push(
                        now,
                        Command::SpawnPed(CreatePedestrian {
                            id: person.ped,
                            speed: person.ped_speed,
                            start,
                            goal: walk_to,
                            path,
                            req,
                            trip,
                            person: person.id,
                        }),
                    );
                } else {
                    self.events.push(Event::Alert(
                        AlertLocation::Person(person.id),
                        format!("UsingTransit trip couldn't find the first path {}", req),
                    ));
                    self.abort_trip(now, trip, None, parking, scheduler, map);
                }
            }
            TripSpec::Remote {
                trip_time, from, ..
            } => {
                assert_eq!(person.state, PersonState::OffMap);
                person.state = PersonState::Trip(trip);
                self.events
                    .push(Event::PersonLeavesRemoteBuilding(person.id, from));
                scheduler.push(now + trip_time, Command::FinishRemoteTrip(trip));
                self.events.push(Event::TripPhaseStarting(
                    trip,
                    person.id,
                    None,
                    TripPhaseType::Remote,
                ));
            }
        }
    }

    pub fn all_arrivals_at_border(&self, at: IntersectionID) -> Vec<(Time, AgentType)> {
        let mut times = Vec::new();
        for t in &self.trips {
            if t.aborted || t.cancelled {
                continue;
            }
            if let TripEndpoint::Border(i, _) = t.info.start {
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
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Trip {
    id: TripID,
    info: TripInfo,
    started: bool,
    finished_at: Option<Time>,
    total_blocked_time: Duration,
    aborted: bool,
    cancelled: bool,
    legs: VecDeque<TripLeg>,
    person: PersonID,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct TripInfo {
    // Scheduled departure; the start may be delayed if the previous trip is taking too long.
    pub departure: Time,
    pub mode: TripMode,
    pub start: TripEndpoint,
    pub end: TripEndpoint,
    // Did a ScenarioModifier apply to this?
    pub modified: bool,
}

impl Trip {
    // Returns true if this succeeds. If not, trip aborted.
    fn spawn_ped(
        &self,
        now: Time,
        start: SidewalkSpot,
        person: &Person,
        map: &Map,
        scheduler: &mut Scheduler,
        events: &mut Vec<Event>,
    ) -> bool {
        let walk_to = match self.legs[0] {
            TripLeg::Walk(ref to) => to.clone(),
            _ => unreachable!(),
        };

        let req = PathRequest {
            start: start.sidewalk_pos,
            end: walk_to.sidewalk_pos,
            constraints: PathConstraints::Pedestrian,
        };
        let path = if let Some(p) = map.pathfind(req.clone()) {
            p
        } else {
            events.push(Event::Alert(
                AlertLocation::Person(self.person),
                format!(
                    "Aborting {} because no path for the walking portion! {:?} to {:?}",
                    self.id, start, walk_to
                ),
            ));
            return false;
        };

        scheduler.push(
            now,
            Command::SpawnPed(CreatePedestrian {
                id: person.ped,
                speed: person.ped_speed,
                start,
                goal: walk_to,
                path,
                req,
                trip: self.id,
                person: self.person,
            }),
        );
        true
    }

    fn assert_walking_leg(&mut self, goal: SidewalkSpot) {
        match self.legs.pop_front() {
            Some(TripLeg::Walk(spot)) => {
                assert_eq!(goal, spot);
            }
            _ => unreachable!(),
        }
    }
}

// These don't specify where the leg starts, since it might be unknown -- like when we drive and
// don't know where we'll wind up parking.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TripLeg {
    Walk(SidewalkSpot),
    // A person may own many vehicles, so specify which they use
    Drive(CarID, DrivingGoal),
    // Maybe get off at a stop, maybe ride off-map
    RideBus(BusRouteID, Option<BusStopID>),
    Remote(OffMapLocation),
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

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TripEndpoint {
    Bldg(BuildingID),
    Border(IntersectionID, Option<OffMapLocation>),
}

pub enum TripResult<T> {
    Ok(T),
    ModeChange,
    TripDone,
    TripDoesntExist,
    TripNotStarted,
    TripAborted,
    TripCancelled,
    RemoteTrip,
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
            TripResult::TripAborted => TripResult::TripAborted,
            TripResult::TripCancelled => TripResult::TripCancelled,
            TripResult::RemoteTrip => TripResult::RemoteTrip,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Person {
    pub id: PersonID,
    pub orig_id: Option<OrigPersonID>,
    pub trips: Vec<TripID>,
    // TODO home
    pub state: PersonState,

    pub ped: PedestrianID,
    pub ped_speed: Speed,
    // Both cars and bikes
    pub vehicles: Vec<Vehicle>,

    delayed_trips: Vec<(TripID, TripSpec, Option<PathRequest>, Option<Path>)>,
    on_bus: Option<CarID>,
}

impl Person {
    pub(crate) fn get_vehicle(&self, id: CarID) -> Vehicle {
        self.vehicles.iter().find(|v| v.id == id).unwrap().clone()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum PersonState {
    Trip(TripID),
    Inside(BuildingID),
    OffMap,
}

impl TripEndpoint {
    pub(crate) fn start_sidewalk_spot(&self, map: &Map) -> Option<SidewalkSpot> {
        match self {
            TripEndpoint::Bldg(b) => Some(SidewalkSpot::building(*b, map)),
            TripEndpoint::Border(i, origin) => {
                SidewalkSpot::start_at_border(*i, origin.clone(), map)
            }
        }
    }

    pub(crate) fn end_sidewalk_spot(&self, map: &Map) -> Option<SidewalkSpot> {
        match self {
            TripEndpoint::Bldg(b) => Some(SidewalkSpot::building(*b, map)),
            TripEndpoint::Border(i, destination) => {
                SidewalkSpot::end_at_border(*i, destination.clone(), map)
            }
        }
    }

    pub(crate) fn driving_goal(
        &self,
        constraints: PathConstraints,
        map: &Map,
    ) -> Option<DrivingGoal> {
        match self {
            TripEndpoint::Bldg(b) => Some(DrivingGoal::ParkNear(*b)),
            TripEndpoint::Border(i, destination) => DrivingGoal::end_at_border(
                map.get_i(*i).some_incoming_road(map)?,
                constraints,
                destination.clone(),
                map,
            ),
        }
    }
}

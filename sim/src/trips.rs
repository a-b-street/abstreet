use crate::scheduler::CommandType;
use crate::{
    AgentID, CarID, Command, CreateCar, CreatePedestrian, DrivingGoal, Event, ParkingSimState,
    ParkingSpot, PedestrianID, PersonID, Scheduler, SidewalkPOI, SidewalkSpot, TransitSimState,
    TripID, TripPhaseType, Vehicle, VehicleType, WalkingSimState,
};
use abstutil::{deserialize_btreemap, serialize_btreemap, Counter};
use geom::{Duration, Speed, Time};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, Map, PathConstraints, PathRequest, Position,
};
use serde_derive::{Deserialize, Serialize};
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

    events: Vec<Event>,
}

impl TripManager {
    pub fn new() -> TripManager {
        TripManager {
            trips: Vec::new(),
            people: Vec::new(),
            active_trip_mode: BTreeMap::new(),
            unfinished_trips: 0,
            events: Vec::new(),
        }
    }

    pub fn new_person(&mut self, id: PersonID, has_car: bool) {
        assert_eq!(id.0, self.people.len());
        self.people.push(Person {
            id,
            trips: Vec::new(),
            state: PersonState::Limbo,
            has_car,
        });
    }
    pub fn random_person(&mut self, has_car: bool) -> PersonID {
        let id = PersonID(self.people.len());
        self.new_person(id, has_car);
        id
    }

    pub fn new_trip(
        &mut self,
        person: PersonID,
        spawned_at: Time,
        start: TripEndpoint,
        legs: Vec<TripLeg>,
    ) -> TripID {
        assert!(!legs.is_empty());
        // TODO Make sure the legs constitute a valid state machine.

        let id = TripID(self.trips.len());
        let mut mode = TripMode::Walk;
        for l in &legs {
            match l {
                TripLeg::Walk(_, _, ref spot) => {
                    if let SidewalkPOI::DeferredParkingSpot(_, _) = spot.connection {
                        mode = TripMode::Drive;
                    }
                }
                TripLeg::Drive(ref vehicle, _) => {
                    mode = TripMode::Drive;
                    if vehicle.vehicle_type == VehicleType::Bike {
                        mode = TripMode::Bike;
                    }
                }
                TripLeg::RideBus(_, _, _) => {
                    mode = TripMode::Transit;
                }
            }
        }
        let end = match legs.last() {
            Some(TripLeg::Walk(_, _, ref spot)) => match spot.connection {
                SidewalkPOI::Building(b) => TripEndpoint::Bldg(b),
                SidewalkPOI::Border(i) => TripEndpoint::Border(i),
                SidewalkPOI::DeferredParkingSpot(_, ref goal) => match goal {
                    DrivingGoal::ParkNear(b) => TripEndpoint::Bldg(*b),
                    DrivingGoal::Border(i, _) => TripEndpoint::Border(*i),
                },
                _ => unreachable!(),
            },
            Some(TripLeg::Drive(_, ref goal)) => match goal {
                DrivingGoal::ParkNear(b) => TripEndpoint::Bldg(*b),
                DrivingGoal::Border(i, _) => TripEndpoint::Border(*i),
            },
            _ => unreachable!(),
        };
        let trip = Trip {
            id,
            person,
            spawned_at,
            finished_at: None,
            total_blocked_time: Duration::ZERO,
            aborted: false,
            mode,
            legs: VecDeque::from(legs),
            start,
            end,
        };
        self.unfinished_trips += 1;
        let person = &mut self.people[trip.person.0];
        if person.trips.is_empty() {
            person.state = match trip.start {
                TripEndpoint::Bldg(b) => {
                    self.events
                        .push(Event::PersonEntersBuilding(trip.person, b));
                    PersonState::Inside(b)
                }
                TripEndpoint::Border(_) => PersonState::OffMap,
            };
        }
        if let Some(t) = person.trips.last() {
            // TODO If it's exactly ==, what?! See the ID.
            if self.trips[t.0].spawned_at > trip.spawned_at {
                panic!(
                    "{} has a trip starting at {}, then one at {}",
                    person.id, self.trips[t.0].spawned_at, trip.spawned_at
                );
            }
        }
        person.trips.push(id);
        self.trips.push(trip);
        id
    }

    pub fn dynamically_override_legs(&mut self, id: TripID, legs: Vec<TripLeg>) {
        let trip = &mut self.trips[id.0];
        trip.legs = VecDeque::from(legs);
        // This is only for peds using a previously unknown parked car
        trip.mode = TripMode::Drive;
    }

    pub fn agent_starting_trip_leg(&mut self, agent: AgentID, t: TripID) {
        assert!(!self.active_trip_mode.contains_key(&agent));
        // TODO ensure a trip only has one active agent (aka, not walking and driving at the same
        // time)
        self.active_trip_mode.insert(agent, t);
        let trip = &self.trips[t.0];
        self.people[trip.person.0].state = PersonState::Trip(t);
    }

    pub fn car_reached_parking_spot(
        &mut self,
        now: Time,
        car: CarID,
        spot: ParkingSpot,
        blocked_time: Duration,
        map: &Map,
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        self.events.push(Event::CarReachedParkingSpot(car, spot));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        trip.total_blocked_time += blocked_time;

        match trip.legs.pop_front() {
            Some(TripLeg::Drive(vehicle, DrivingGoal::ParkNear(_))) => assert_eq!(car, vehicle.id),
            _ => unreachable!(),
        };

        match &trip.legs[0] {
            TripLeg::Walk(_, _, to) => match (spot, &to.connection) {
                (ParkingSpot::Offstreet(b1, _), SidewalkPOI::Building(b2)) if b1 == *b2 => {
                    // Do the relevant parts of ped_reached_parking_spot.
                    assert_eq!(trip.legs.len(), 1);
                    assert!(!trip.finished_at.is_some());
                    trip.finished_at = Some(now);
                    self.unfinished_trips -= 1;
                    self.events.push(Event::TripFinished {
                        trip: trip.id,
                        mode: trip.mode,
                        total_time: now - trip.spawned_at,
                        blocked_time: trip.total_blocked_time,
                    });
                    self.people[trip.person.0].state = PersonState::Inside(b1);
                    self.events
                        .push(Event::PersonEntersBuilding(trip.person, b1));
                    return;
                }
                _ => {}
            },
            _ => unreachable!(),
        };

        if !trip.spawn_ped(
            now,
            SidewalkSpot::parking_spot(spot, map, parking),
            map,
            scheduler,
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
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        self.events.push(Event::PedReachedParkingSpot(ped, spot));
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        trip.assert_walking_leg(ped, SidewalkSpot::parking_spot(spot, map, parking));
        let (car, drive_to) = match trip.legs[0] {
            TripLeg::Drive(ref vehicle, ref to) => (vehicle.id, to.clone()),
            _ => unreachable!(),
        };
        let parked_car = parking.get_car_at_spot(spot).unwrap();
        assert_eq!(parked_car.vehicle.id, car);

        let mut start = parking.spot_to_driving_pos(parked_car.spot, &parked_car.vehicle, map);
        if let ParkingSpot::Offstreet(_, _) = spot {
            // Actually, to unpark, the car's front should be where it'll wind up at the end.
            start = Position::new(start.lane(), start.dist_along() + parked_car.vehicle.length);
        }
        let end = drive_to.goal_pos(PathConstraints::Car, map);
        let req = PathRequest {
            start,
            end,
            constraints: PathConstraints::Car,
        };
        let path = if let Some(p) = map.pathfind(req.clone()) {
            p
        } else {
            println!(
                "Aborting {} at {} because no path for the car portion! {} to {}",
                trip.id, now, start, end
            );
            self.unfinished_trips -= 1;
            trip.aborted = true;
            self.events.push(Event::TripAborted(trip.id, trip.mode));
            self.people[trip.person.0].state = PersonState::Limbo;
            return;
        };

        let router = drive_to.make_router(path, map, parked_car.vehicle.vehicle_type);
        scheduler.push(
            now,
            Command::SpawnCar(
                CreateCar::for_parked_car(
                    parked_car.clone(),
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
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        trip.assert_walking_leg(ped, spot.clone());
        let (vehicle, drive_to) = match trip.legs[0] {
            TripLeg::Drive(ref vehicle, ref to) => (vehicle.clone(), to.clone()),
            _ => unreachable!(),
        };
        let driving_pos = match spot.connection {
            SidewalkPOI::BikeRack(p) => p,
            _ => unreachable!(),
        };

        let end = drive_to.goal_pos(PathConstraints::Bike, map);
        let req = PathRequest {
            start: driving_pos,
            end,
            constraints: PathConstraints::Bike,
        };
        let path = if let Some(p) = map.pathfind(req.clone()) {
            p
        } else {
            println!(
                "Aborting {} at {} because no path for the bike portion! {} to {}",
                trip.id, now, driving_pos, end
            );
            self.unfinished_trips -= 1;
            trip.aborted = true;
            self.events.push(Event::TripAborted(trip.id, trip.mode));
            self.people[trip.person.0].state = PersonState::Limbo;
            return;
        };

        let router = drive_to.make_router(path, map, vehicle.vehicle_type);
        scheduler.push(
            now,
            Command::SpawnCar(
                CreateCar::for_appearing(vehicle, driving_pos, router, req, trip.id, trip.person),
                true,
            ),
        );
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
            Some(TripLeg::Drive(vehicle, DrivingGoal::ParkNear(_))) => assert_eq!(vehicle.id, bike),
            _ => unreachable!(),
        };

        if !trip.spawn_ped(now, bike_rack, map, scheduler) {
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
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        trip.assert_walking_leg(ped, SidewalkSpot::building(bldg, map));
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
        self.unfinished_trips -= 1;
        self.events.push(Event::TripFinished {
            trip: trip.id,
            mode: trip.mode,
            total_time: now - trip.spawned_at,
            blocked_time: trip.total_blocked_time,
        });
        self.people[trip.person.0].state = PersonState::Inside(bldg);
        self.events
            .push(Event::PersonEntersBuilding(trip.person, bldg));
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
            TripLeg::Walk(p, _, ref spot) => {
                assert_eq!(p, ped);
                assert_eq!(*spot, SidewalkSpot::bus_stop(stop, map));
            }
            _ => unreachable!(),
        }
        match trip.legs[1] {
            TripLeg::RideBus(_, route, stop2) => {
                self.events.push(Event::TripPhaseStarting(
                    trip.id,
                    trip.person,
                    trip.mode,
                    None,
                    TripPhaseType::WaitingForBus(route, stop),
                ));
                if transit.ped_waiting_for_bus(
                    now,
                    ped,
                    trip.id,
                    trip.person,
                    stop,
                    route,
                    stop2,
                    map,
                ) {
                    trip.legs.pop_front();
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
        blocked_time: Duration,
        walking: &mut WalkingSimState,
    ) -> (TripID, PersonID) {
        // TODO Make sure canonical pt is the bus while the ped is riding it
        let trip = &mut self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];
        trip.total_blocked_time += blocked_time;

        trip.legs.pop_front();
        walking.ped_boarded_bus(now, ped);
        (trip.id, trip.person)
    }

    // TODO Need to characterize delay the bus experienced
    pub fn ped_left_bus(
        &mut self,
        now: Time,
        ped: PedestrianID,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        let start = match trip.legs.pop_front().unwrap() {
            TripLeg::RideBus(_, _, stop) => SidewalkSpot::bus_stop(stop, map),
            _ => unreachable!(),
        };

        if !trip.spawn_ped(now, start, map, scheduler) {
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
    ) {
        self.events.push(Event::PedReachedBorder(ped, i));
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.total_blocked_time += blocked_time;

        trip.assert_walking_leg(ped, SidewalkSpot::end_at_border(i, map).unwrap());
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
        self.unfinished_trips -= 1;
        self.events.push(Event::TripFinished {
            trip: trip.id,
            mode: trip.mode,
            total_time: now - trip.spawned_at,
            blocked_time: trip.total_blocked_time,
        });
        self.people[trip.person.0].state = PersonState::OffMap;
    }

    pub fn car_or_bike_reached_border(
        &mut self,
        now: Time,
        car: CarID,
        i: IntersectionID,
        blocked_time: Duration,
    ) {
        self.events.push(Event::CarOrBikeReachedBorder(car, i));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        trip.total_blocked_time += blocked_time;

        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(_, DrivingGoal::Border(int, _)) => assert_eq!(i, int),
            _ => unreachable!(),
        };
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
        self.unfinished_trips -= 1;
        self.events.push(Event::TripFinished {
            trip: trip.id,
            mode: trip.mode,
            total_time: now - trip.spawned_at,
            blocked_time: trip.total_blocked_time,
        });
        self.people[trip.person.0].state = PersonState::OffMap;
    }

    pub fn abort_trip_failed_start(&mut self, id: TripID) {
        let trip = &mut self.trips[id.0];
        trip.aborted = true;
        self.unfinished_trips -= 1;
        // TODO Urgh, hack. Initialization order is now quite complicated.
        self.people[trip.person.0].state = PersonState::Limbo;
        self.events.push(Event::TripAborted(id, trip.mode));
    }

    pub fn abort_trip_impossible_parking(&mut self, car: CarID) {
        let id = self.active_trip_mode.remove(&AgentID::Car(car)).unwrap();
        let trip = &mut self.trips[id.0];
        trip.aborted = true;
        self.unfinished_trips -= 1;
        self.events.push(Event::TripAborted(trip.id, trip.mode));
        self.people[trip.person.0].state = PersonState::Limbo;
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

        let a = match &trip.legs[0] {
            TripLeg::Walk(id, _, _) => AgentID::Pedestrian(*id),
            TripLeg::Drive(vehicle, _) => AgentID::Car(vehicle.id),
            // TODO Should be the bus, but apparently transit sim tracks differently?
            TripLeg::RideBus(ped, _, _) => AgentID::Pedestrian(*ped),
        };
        if self.active_trip_mode.get(&a) == Some(&id) {
            TripResult::Ok(a)
        } else {
            TripResult::TripNotStarted
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

    pub fn num_trips(&self) -> (usize, usize, BTreeMap<TripMode, usize>) {
        let mut cnt = Counter::new();
        for a in self.active_trip_mode.keys() {
            // TODO This conflates bus riders and buses...
            cnt.inc(TripMode::from_agent(*a));
        }
        let per_mode = TripMode::all()
            .into_iter()
            .map(|k| (k, cnt.get(k)))
            .collect();
        (
            self.trips.len() - self.unfinished_trips,
            self.unfinished_trips,
            per_mode,
        )
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
                PersonState::Limbo => {}
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

    pub fn trip_info(&self, id: TripID) -> (Time, TripEndpoint, TripEndpoint, TripMode) {
        let t = &self.trips[id.0];
        (t.spawned_at, t.start.clone(), t.end.clone(), t.mode)
    }
    pub fn finished_trip_time(&self, id: TripID) -> Option<(Duration, Duration)> {
        let t = &self.trips[id.0];
        Some((t.finished_at? - t.spawned_at, t.total_blocked_time))
    }

    pub fn count_trips(&self, endpt: TripEndpoint, now: Time) -> TripCount {
        let mut cnt = TripCount {
            from_aborted: Vec::new(),
            from_in_progress: Vec::new(),
            from_completed: Vec::new(),
            from_unstarted: Vec::new(),
            to_aborted: Vec::new(),
            to_in_progress: Vec::new(),
            to_completed: Vec::new(),
            to_unstarted: Vec::new(),
        };
        for trip in &self.trips {
            if trip.start == endpt {
                if trip.aborted {
                    cnt.from_aborted.push(trip.id);
                } else if trip.finished_at.is_some() {
                    cnt.from_completed.push(trip.id);
                } else if now >= trip.spawned_at {
                    cnt.from_in_progress.push(trip.id);
                } else {
                    cnt.from_unstarted.push(trip.id);
                }
            }
            // One trip might could towards both!
            if trip.end == endpt {
                if trip.aborted {
                    cnt.to_aborted.push(trip.id);
                } else if trip.finished_at.is_some() {
                    cnt.to_completed.push(trip.id);
                } else if now >= trip.spawned_at {
                    cnt.to_in_progress.push(trip.id);
                } else {
                    cnt.to_unstarted.push(trip.id);
                }
            }
        }
        cnt
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

    pub fn get_person(&self, p: PersonID) -> &Person {
        &self.people[p.0]
    }
    pub fn get_all_people(&self) -> &Vec<Person> {
        &self.people
    }

    pub fn trip_to_person(&self, id: TripID) -> PersonID {
        self.trips[id.0].person
    }

    pub fn cancel_future_trips(&mut self, now: Time, p: PersonID, scheduler: &mut Scheduler) {
        let person = &self.people[p.0];
        println!("Aborting trips by {} ({:?}) after {}", p, person.state, now);
        for t in &person.trips {
            let trip = &mut self.trips[t.0];
            // TODO Make sure we're only cancelling unstarted trips. There's a race condition where
            // we schedule the CancelTrips command exactly at the time a trip is supposed to start.
            if trip.spawned_at > now {
                println!("  Aborting trip {} at {}", trip.id, trip.spawned_at);
                self.unfinished_trips -= 1;
                trip.aborted = true;
                self.events.push(Event::TripAborted(trip.id, trip.mode));

                scheduler.must_cancel_by_type(match trip.legs[0] {
                    TripLeg::Walk(ped, _, _) | TripLeg::RideBus(ped, _, _) => CommandType::Ped(ped),
                    TripLeg::Drive(ref vehicle, _) => CommandType::Car(vehicle.id),
                });
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Trip {
    id: TripID,
    spawned_at: Time,
    finished_at: Option<Time>,
    total_blocked_time: Duration,
    aborted: bool,
    legs: VecDeque<TripLeg>,
    mode: TripMode,
    start: TripEndpoint,
    end: TripEndpoint,
    person: PersonID,
}

impl Trip {
    // Returns true if this succeeds. If not, trip aborted.
    fn spawn_ped(
        &self,
        now: Time,
        start: SidewalkSpot,
        map: &Map,
        scheduler: &mut Scheduler,
    ) -> bool {
        let (ped, speed, walk_to) = match self.legs[0] {
            TripLeg::Walk(ped, speed, ref to) => (ped, speed, to.clone()),
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
            println!(
                "Aborting {} at {} because no path for the walking portion! {:?} to {:?}",
                self.id, now, start, walk_to
            );
            return false;
        };

        scheduler.push(
            now,
            Command::SpawnPed(CreatePedestrian {
                id: ped,
                speed,
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

    fn assert_walking_leg(&mut self, ped: PedestrianID, goal: SidewalkSpot) {
        match self.legs.pop_front() {
            Some(TripLeg::Walk(p, _, spot)) => {
                assert_eq!(ped, p);
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
    Walk(PedestrianID, Speed, SidewalkSpot),
    Drive(Vehicle, DrivingGoal),
    RideBus(PedestrianID, BusRouteID, BusStopID),
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

    pub(crate) fn from_agent(id: AgentID) -> TripMode {
        match id {
            AgentID::Pedestrian(_) => TripMode::Walk,
            AgentID::Car(id) => match id.1 {
                VehicleType::Car => TripMode::Drive,
                VehicleType::Bike => TripMode::Bike,
                // Little confusing; this means buses, not bus riders.
                VehicleType::Bus => TripMode::Transit,
            },
        }
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
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TripEndpoint {
    Bldg(BuildingID),
    Border(IntersectionID),
}

pub enum TripResult<T> {
    Ok(T),
    ModeChange,
    TripDone,
    TripDoesntExist,
    TripNotStarted,
    TripAborted,
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
        }
    }
}

// TODO Misnomer now
pub struct TripCount {
    pub from_aborted: Vec<TripID>,
    pub from_in_progress: Vec<TripID>,
    pub from_completed: Vec<TripID>,
    pub from_unstarted: Vec<TripID>,
    pub to_aborted: Vec<TripID>,
    pub to_in_progress: Vec<TripID>,
    pub to_completed: Vec<TripID>,
    pub to_unstarted: Vec<TripID>,
}

impl TripCount {
    pub fn describe(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.from_completed.is_empty() || !self.to_completed.is_empty() {
            lines.push(format!(
                "Finished trips: {} from here, {} to here",
                self.from_completed.len(),
                self.to_completed.len()
            ));
        }
        if !self.from_in_progress.is_empty() || !self.to_in_progress.is_empty() {
            lines.push(format!(
                "In-progress trips: {} from here, {} to here",
                self.from_in_progress.len(),
                self.to_in_progress.len()
            ));
        }
        if !self.from_unstarted.is_empty() || !self.to_unstarted.is_empty() {
            lines.push(format!(
                "Future trips: {} from here, {} to here",
                self.from_unstarted.len(),
                self.to_unstarted.len()
            ));
        }
        if !self.from_aborted.is_empty() || !self.to_aborted.is_empty() {
            lines.push(format!(
                "Aborted trips: {} from here, {} to here",
                self.from_aborted.len(),
                self.to_aborted.len()
            ));
        }
        lines
    }
}

// TODO General weirdness right now: we operate based on trips, and the people are side-effects. So
// one "person" might start their second trip before finishing their first.

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Person {
    pub id: PersonID,
    pub trips: Vec<TripID>,
    // TODO home
    pub state: PersonState,
    pub has_car: bool,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum PersonState {
    Trip(TripID),
    Inside(BuildingID),
    OffMap,
    // One of the trips was aborted. Silently eat them from the exposed counters. :\
    Limbo,
}

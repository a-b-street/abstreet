use crate::{
    AgentID, CarID, Command, CreateCar, CreatePedestrian, DrivingGoal, Event, ParkingSimState,
    ParkingSpot, PedestrianID, Scheduler, SidewalkPOI, SidewalkSpot, TransitSimState, TripID,
    Vehicle, WalkingSimState,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Duration, Speed};
use map_model::{BuildingID, BusRouteID, BusStopID, IntersectionID, Map, PathRequest, Position};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct TripManager {
    trips: Vec<Trip>,
    // For quick lookup of active agents
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    active_trip_mode: BTreeMap<AgentID, TripID>,
    num_bus_trips: usize,
    unfinished_trips: usize,

    events: Vec<Event>,
}

impl TripManager {
    pub fn new() -> TripManager {
        TripManager {
            trips: Vec::new(),
            active_trip_mode: BTreeMap::new(),
            num_bus_trips: 0,
            unfinished_trips: 0,
            events: Vec::new(),
        }
    }

    pub fn new_trip(
        &mut self,
        spawned_at: Duration,
        start: TripStart,
        legs: Vec<TripLeg>,
    ) -> TripID {
        assert!(!legs.is_empty());
        // TODO Make sure the legs constitute a valid state machine.

        let id = TripID(self.trips.len());
        let mut mode = TripMode::Walk;
        for l in &legs {
            match l {
                TripLeg::Walk(_, _, _) => {}
                TripLeg::Drive(_, _) => {
                    mode = TripMode::Drive;
                }
                TripLeg::RideBus(_, _, _) => {
                    mode = TripMode::Transit;
                }
                TripLeg::ServeBusRoute(_, _) => {
                    // Confusing, because Transit usually means riding transit. But bus trips will
                    // never get returned in FinishedTrips anyway.
                    mode = TripMode::Transit;
                }
            }
        }
        let end = match legs.last() {
            Some(TripLeg::Walk(_, _, ref spot)) => match spot.connection {
                SidewalkPOI::Building(b) => TripEnd::Bldg(b),
                SidewalkPOI::Border(i) => TripEnd::Border(i),
                SidewalkPOI::DeferredParkingSpot(_, ref goal) => match goal {
                    DrivingGoal::ParkNear(b) => TripEnd::Bldg(*b),
                    DrivingGoal::Border(i, _) => TripEnd::Border(*i),
                },
                _ => unreachable!(),
            },
            Some(TripLeg::Drive(_, ref goal)) => match goal {
                DrivingGoal::ParkNear(b) => TripEnd::Bldg(*b),
                DrivingGoal::Border(i, _) => TripEnd::Border(*i),
            },
            Some(TripLeg::ServeBusRoute(_, route)) => TripEnd::ServeBusRoute(*route),
            _ => unreachable!(),
        };
        let trip = Trip {
            id,
            spawned_at,
            finished_at: None,
            aborted: false,
            mode,
            legs: VecDeque::from(legs),
            start,
            end,
        };
        if !trip.is_bus_trip() {
            self.unfinished_trips += 1;
        }
        self.trips.push(trip);
        id
    }

    pub fn dynamically_override_legs(&mut self, id: TripID, legs: Vec<TripLeg>) {
        let trip = &mut self.trips[id.0];
        trip.legs = VecDeque::from(legs);
        // This is only for peds using a previously unknown parked car
        trip.mode = TripMode::Drive;
    }

    pub fn agent_starting_trip_leg(&mut self, agent: AgentID, trip: TripID) {
        assert!(!self.active_trip_mode.contains_key(&agent));
        // TODO ensure a trip only has one active agent (aka, not walking and driving at the same
        // time)
        self.active_trip_mode.insert(agent, trip);
        if self.trips[trip.0].is_bus_trip() {
            self.num_bus_trips += 1;
        }
    }

    pub fn car_reached_parking_spot(
        &mut self,
        now: Duration,
        car: CarID,
        spot: ParkingSpot,
        map: &Map,
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        self.events.push(Event::CarReachedParkingSpot(car, spot));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];

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
        now: Duration,
        ped: PedestrianID,
        spot: ParkingSpot,
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
        let end = drive_to.goal_pos(map);
        let path = if let Some(p) = map.pathfind(PathRequest {
            start,
            end,
            can_use_bus_lanes: false,
            can_use_bike_lanes: false,
        }) {
            p
        } else {
            println!(
                "Aborting {} at {} because no path for the car portion! {:?} to {:?}",
                trip.id, now, start, end
            );
            self.unfinished_trips -= 1;
            trip.aborted = true;
            return;
        };

        let router = drive_to.make_router(path, map, parked_car.vehicle.vehicle_type);
        scheduler.push(
            now,
            Command::SpawnCar(
                CreateCar::for_parked_car(parked_car.clone(), router, start.dist_along(), trip.id),
                true,
            ),
        );
    }

    pub fn ped_ready_to_bike(
        &mut self,
        now: Duration,
        ped: PedestrianID,
        spot: SidewalkSpot,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];

        trip.assert_walking_leg(ped, spot.clone());
        let (vehicle, drive_to) = match trip.legs[0] {
            TripLeg::Drive(ref vehicle, ref to) => (vehicle.clone(), to.clone()),
            _ => unreachable!(),
        };
        let driving_pos = match spot.connection {
            SidewalkPOI::BikeRack(p) => p,
            _ => unreachable!(),
        };

        let end = drive_to.goal_pos(map);
        let path = if let Some(p) = map.pathfind(PathRequest {
            start: driving_pos,
            end,
            can_use_bus_lanes: false,
            can_use_bike_lanes: true,
        }) {
            p
        } else {
            println!(
                "Aborting {} at {} because no path for the bike portion! {:?} to {:?}",
                trip.id, now, driving_pos, end
            );
            self.unfinished_trips -= 1;
            trip.aborted = true;
            return;
        };

        let router = drive_to.make_router(path, map, vehicle.vehicle_type);
        scheduler.push(
            now,
            Command::SpawnCar(
                CreateCar::for_appearing(vehicle, driving_pos, router, trip.id),
                true,
            ),
        );
    }

    pub fn bike_reached_end(
        &mut self,
        now: Duration,
        bike: CarID,
        bike_rack: SidewalkSpot,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        self.events.push(Event::BikeStoppedAtSidewalk(
            bike,
            bike_rack.sidewalk_pos.lane(),
        ));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(bike)).unwrap().0];

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
        now: Duration,
        ped: PedestrianID,
        bldg: BuildingID,
        map: &Map,
    ) {
        self.events.push(Event::PedReachedBuilding(ped, bldg));
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.assert_walking_leg(ped, SidewalkSpot::building(bldg, map));
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
        self.unfinished_trips -= 1;
    }

    // If no route is returned, the pedestrian boarded a bus immediately.
    pub fn ped_reached_bus_stop(
        &mut self,
        ped: PedestrianID,
        stop: BusStopID,
        map: &Map,
        transit: &mut TransitSimState,
    ) -> Option<BusRouteID> {
        self.events.push(Event::PedReachedBusStop(ped, stop));
        let trip = &mut self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];
        match trip.legs[0] {
            TripLeg::Walk(p, _, ref spot) => {
                assert_eq!(p, ped);
                assert_eq!(*spot, SidewalkSpot::bus_stop(stop, map));
            }
            _ => unreachable!(),
        }
        match trip.legs[1] {
            TripLeg::RideBus(_, route, stop2) => {
                if transit.ped_waiting_for_bus(ped, stop, route, stop2) {
                    trip.legs.pop_front();
                    None
                } else {
                    Some(route)
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn ped_boarded_bus(&mut self, ped: PedestrianID, walking: &mut WalkingSimState) {
        // TODO Make sure canonical pt is the bus while the ped is riding it
        let trip = &mut self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];
        trip.legs.pop_front();
        walking.ped_boarded_bus(ped);
    }

    pub fn ped_left_bus(
        &mut self,
        now: Duration,
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
        now: Duration,
        ped: PedestrianID,
        i: IntersectionID,
        map: &Map,
    ) {
        self.events.push(Event::PedReachedBorder(ped, i));
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];
        trip.assert_walking_leg(ped, SidewalkSpot::end_at_border(i, map).unwrap());
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
        self.unfinished_trips -= 1;
    }

    pub fn car_or_bike_reached_border(&mut self, now: Duration, car: CarID, i: IntersectionID) {
        self.events.push(Event::CarOrBikeReachedBorder(car, i));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(_, DrivingGoal::Border(int, _)) => assert_eq!(i, int),
            _ => unreachable!(),
        };
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
        self.unfinished_trips -= 1;
    }

    pub fn abort_trip_failed_start(&mut self, id: TripID) {
        self.trips[id.0].aborted = true;
        if !self.trips[id.0].is_bus_trip() {
            self.unfinished_trips -= 1;
        }
    }

    pub fn abort_trip_impossible_parking(&mut self, car: CarID) {
        let trip = self.active_trip_mode.remove(&AgentID::Car(car)).unwrap();
        assert!(!self.trips[trip.0].is_bus_trip());
        self.trips[trip.0].aborted = true;
        self.unfinished_trips -= 1;
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

        if trip.finished_at.is_some() || trip.aborted {
            return TripResult::TripDone;
        }

        match &trip.legs[0] {
            TripLeg::Walk(id, _, _) => TripResult::Ok(AgentID::Pedestrian(*id)),
            TripLeg::Drive(vehicle, _) => TripResult::Ok(AgentID::Car(vehicle.id)),
            // TODO Should be the bus, but apparently transit sim tracks differently?
            TripLeg::RideBus(ped, _, _) => TripResult::Ok(AgentID::Pedestrian(*ped)),
            TripLeg::ServeBusRoute(id, _) => TripResult::Ok(AgentID::Car(*id)),
        }
    }

    // This will be None for parked cars
    pub fn agent_to_trip(&self, id: AgentID) -> Option<TripID> {
        self.active_trip_mode.get(&id).cloned()
    }

    pub fn tooltip_lines(&self, id: AgentID) -> Vec<String> {
        // Only called for agents that _should_ have trips
        let trip = &self.trips[self.active_trip_mode[&id].0];
        vec![format!(
            "{} has goal {:?}",
            trip.id,
            trip.legs.back().unwrap()
        )]
    }

    // (active not including buses, unfinished, buses)
    pub fn num_trips(&self) -> (usize, usize, usize) {
        (
            self.active_trip_mode.len() - self.num_bus_trips,
            self.unfinished_trips,
            self.num_bus_trips,
        )
    }

    pub fn get_finished_trips(&self) -> FinishedTrips {
        let mut result = FinishedTrips {
            unfinished_trips: self.unfinished_trips,
            aborted_trips: 0,
            finished_trips: Vec::new(),
        };
        for t in &self.trips {
            if let Some(end) = t.finished_at {
                result
                    .finished_trips
                    .push((t.id, t.mode, end - t.spawned_at));
            } else if t.aborted {
                result.aborted_trips += 1;
            }
        }
        result
    }

    pub fn is_done(&self) -> bool {
        self.unfinished_trips == 0
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        std::mem::replace(&mut self.events, Vec::new())
    }

    pub fn trip_status(&self, id: TripID) -> TripStatus {
        let trip = &self.trips[id.0];
        TripStatus {
            start: trip.start.clone(),
            end: trip.end.clone(),
        }
    }

    // Return trip start time too
    pub fn find_trip_using_car(&self, id: CarID, home: BuildingID) -> Option<(TripID, Duration)> {
        let t = self.trips.iter().find(|t| t.uses_car(id, home))?;
        Some((t.id, t.spawned_at))
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Trip {
    id: TripID,
    spawned_at: Duration,
    finished_at: Option<Duration>,
    aborted: bool,
    legs: VecDeque<TripLeg>,
    mode: TripMode,
    start: TripStart,
    end: TripEnd,
}

impl Trip {
    fn uses_car(&self, id: CarID, home: BuildingID) -> bool {
        self.legs.iter().any(|l| match l {
            TripLeg::Walk(_, _, ref walk_to) => match walk_to.connection {
                SidewalkPOI::DeferredParkingSpot(b, _) => b == home,
                _ => false,
            },
            // No need to look up the contents of a SidewalkPOI::ParkingSpot. If a trip uses a
            // specific parked car, then there'll be a TripLeg::Drive with it already.
            TripLeg::Drive(ref vehicle, _) => vehicle.id == id,
            _ => false,
        })
    }

    fn is_bus_trip(&self) -> bool {
        self.legs.len() == 1
            && match self.legs[0] {
                TripLeg::ServeBusRoute(_, _) => true,
                _ => false,
            }
    }

    // Returns true if this succeeds. If not, trip aborted.
    fn spawn_ped(
        &self,
        now: Duration,
        start: SidewalkSpot,
        map: &Map,
        scheduler: &mut Scheduler,
    ) -> bool {
        let (ped, speed, walk_to) = match self.legs[0] {
            TripLeg::Walk(ped, speed, ref to) => (ped, speed, to.clone()),
            _ => unreachable!(),
        };

        let path = if let Some(p) = map.pathfind(PathRequest {
            start: start.sidewalk_pos,
            end: walk_to.sidewalk_pos,
            can_use_bus_lanes: false,
            can_use_bike_lanes: false,
        }) {
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
                trip: self.id,
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
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum TripLeg {
    Walk(PedestrianID, Speed, SidewalkSpot),
    Drive(Vehicle, DrivingGoal),
    RideBus(PedestrianID, BusRouteID, BusStopID),
    ServeBusRoute(CarID, BusRouteID),
}

// As of a moment in time, not necessarily the end of the simulation
pub struct FinishedTrips {
    pub unfinished_trips: usize,
    pub aborted_trips: usize,
    // (..., ..., time to complete trip)
    pub finished_trips: Vec<(TripID, TripMode, Duration)>,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum TripMode {
    Walk,
    Bike,
    Transit,
    Drive,
}

// TODO Argh no, not more of these variants!

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TripStart {
    Bldg(BuildingID),
    Border(IntersectionID),
    Appearing(Position),
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum TripEnd {
    Bldg(BuildingID),
    Border(IntersectionID),
    // No end!
    ServeBusRoute(BusRouteID),
}

pub struct TripStatus {
    pub start: TripStart,
    pub end: TripEnd,
}

pub enum TripResult<T> {
    Ok(T),
    ModeChange,
    TripDone,
    TripDoesntExist,
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
        }
    }
}

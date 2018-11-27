use abstutil::{deserialize_btreemap, serialize_btreemap};
use driving::DrivingGoal;
use kinematics::Vehicle;
use map_model::BusStopID;
use std::collections::{BTreeMap, VecDeque};
use walking::SidewalkSpot;
use {AgentID, CarID, ParkedCar, PedestrianID, RouteID, ScoreSummary, Tick, TripID};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct TripManager {
    trips: Vec<Trip>,
    // For quick lookup of active agents
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    active_trip_mode: BTreeMap<AgentID, TripID>,
}

impl TripManager {
    pub fn new() -> TripManager {
        TripManager {
            trips: Vec::new(),
            active_trip_mode: BTreeMap::new(),
        }
    }

    // Transitions from spawner
    pub fn agent_starting_trip_leg(&mut self, agent: AgentID, trip: TripID) {
        assert!(!self.active_trip_mode.contains_key(&agent));
        // TODO ensure a trip only has one active agent (aka, not walking and driving at the same
        // time)
        self.active_trip_mode.insert(agent, trip);
    }

    // Where are we walking next?
    pub fn car_reached_parking_spot(&mut self, car: CarID) -> (TripID, PedestrianID, SidewalkSpot) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];

        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(parked, _) => assert_eq!(car, parked.car),
            TripLeg::DriveFromBorder(id, _) => assert_eq!(car, id),
            x => panic!(
                "First trip leg {:?} doesn't match car_reached_parking_spot",
                x
            ),
        };
        // TODO there are only some valid sequences of trips. it'd be neat to guarantee these are
        // valid by construction with a fluent API.
        let walk_to = match trip.legs[0] {
            TripLeg::Walk(ref to) => to,
            ref x => panic!("Next trip leg is {:?}, not walking", x),
        };
        (trip.id, trip.ped.unwrap(), walk_to.clone())
    }

    // Where are we driving next?
    pub fn ped_reached_parking_spot(&mut self, ped: PedestrianID) -> (TripID, DrivingGoal) {
        let trip = &mut self.trips[self
                                       .active_trip_mode
                                       .remove(&AgentID::Pedestrian(ped))
                                       .unwrap()
                                       .0];

        match trip.legs.pop_front().unwrap() {
            TripLeg::Walk(_) => {}
            x => panic!(
                "First trip leg {:?} doesn't match ped_reached_parking_spot",
                x
            ),
        };
        let drive_to = match trip.legs[0] {
            TripLeg::Drive(_, ref to) => to.clone(),
            ref x => panic!("Next trip leg is {:?}, not walking", x),
        };
        (trip.id, drive_to)
    }

    pub fn ped_ready_to_bike(&mut self, ped: PedestrianID) -> (TripID, Vehicle, DrivingGoal) {
        let trip = &mut self.trips[self
                                       .active_trip_mode
                                       .remove(&AgentID::Pedestrian(ped))
                                       .unwrap()
                                       .0];

        match trip.legs.pop_front().unwrap() {
            TripLeg::Walk(_) => {}
            x => panic!("First trip leg {:?} doesn't match ped_ready_to_bike", x),
        };
        let (vehicle, bike_to) = match trip.legs[0] {
            TripLeg::Bike(ref vehicle, ref to) => (vehicle, to),
            ref x => panic!("Next trip leg is {:?}, not biking", x),
        };
        (trip.id, vehicle.clone(), bike_to.clone())
    }

    pub fn bike_reached_end(&mut self, bike: CarID) -> (TripID, PedestrianID, SidewalkSpot) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(bike)).unwrap().0];

        match trip.legs.pop_front().unwrap() {
            TripLeg::Bike { .. } => {}
            x => panic!("First trip leg {:?} doesn't match bike_reached_end", x),
        };
        let walk_to = match trip.legs[0] {
            TripLeg::Walk(ref to) => to,
            ref x => panic!("Next trip leg is {:?}, not walking", x),
        };
        (trip.id, trip.ped.unwrap(), walk_to.clone())
    }

    pub fn ped_reached_building_or_border(&mut self, ped: PedestrianID, now: Tick) {
        let trip = &mut self.trips[self
                                       .active_trip_mode
                                       .remove(&AgentID::Pedestrian(ped))
                                       .unwrap()
                                       .0];
        match trip.legs.pop_front().unwrap() {
            TripLeg::Walk(_) => {}
            x => panic!(
                "Last trip leg {:?} doesn't match ped_reached_building_or_border",
                x
            ),
        };
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
    }

    // Or bike
    pub fn car_reached_border(&mut self, car: CarID, now: Tick) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(_, _) => {}
            TripLeg::DriveFromBorder(_, _) => {}
            TripLeg::Bike(_, _) => {}
            x => panic!("Last trip leg {:?} doesn't match car_reached_border", x),
        };
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
    }

    // Combo query/transition from transit
    pub fn should_ped_board_bus(&mut self, ped: PedestrianID, route: RouteID) -> bool {
        let trip = &mut self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];

        let board = match trip.legs[1] {
            TripLeg::RideBus(r, _) => r == route,
            ref x => panic!("{} is at a bus stop, but next leg is {:?}", ped, x),
        };
        if !board {
            return false;
        }

        // Could assert that the first leg is walking to the right bus stop
        trip.legs.pop_front();
        // Leave active_trip_mode as Pedestrian, since the transit sim tracks passengers as
        // PedestrianIDs.

        true
    }

    pub fn should_ped_leave_bus(&self, ped: PedestrianID, stop: BusStopID) -> bool {
        let trip = &self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];

        match trip.legs[0] {
            TripLeg::RideBus(_, until_stop) => stop == until_stop,
            ref x => panic!("{} is on a bus stop, but first leg is {:?}", ped, x),
        }
    }

    // Where to walk next?
    pub fn ped_finished_bus_ride(&mut self, ped: PedestrianID) -> (TripID, SidewalkSpot) {
        // The spawner will call agent_starting_trip_leg, so briefly remove the active PedestrianID.
        let trip = &mut self.trips[self
                                       .active_trip_mode
                                       .remove(&AgentID::Pedestrian(ped))
                                       .unwrap()
                                       .0];

        match trip.legs.pop_front().unwrap() {
            TripLeg::RideBus(_, _) => {}
            x => panic!("First trip leg {:?} doesn't match ped_finished_bus_ride", x),
        };
        // TODO there are only some valid sequences of trips. it'd be neat to guarantee these are
        // valid by construction with a fluent API.
        let walk_to = match trip.legs[0] {
            TripLeg::Walk(ref to) => to,
            ref x => panic!("Next trip leg is {:?}, not walking", x),
        };
        (trip.id, walk_to.clone())
    }

    // Creation from the interactive part of spawner
    pub fn new_trip(
        &mut self,
        spawned_at: Tick,
        ped: Option<PedestrianID>,
        legs: Vec<TripLeg>,
    ) -> TripID {
        assert!(!legs.is_empty());
        // TODO Make sure the legs constitute a valid state machine.

        let id = TripID(self.trips.len());
        self.trips.push(Trip {
            id,
            spawned_at,
            finished_at: None,
            ped,
            uses_car: legs
                .iter()
                .find(|l| match l {
                    TripLeg::Drive(_, _) => true,
                    TripLeg::DriveFromBorder(_, _) => true,
                    TripLeg::ServeBusRoute(_, _) => true,
                    _ => false,
                }).is_some(),
            legs: VecDeque::from(legs),
        });
        id
    }

    // Query from spawner
    pub fn get_trip_using_car(&self, car: CarID) -> Option<TripID> {
        self.trips
            .iter()
            .find(|t| t.legs.iter().find(|l| l.uses_car(car)).is_some())
            .map(|t| t.id)
    }

    pub fn get_score(&self, now: Tick) -> ScoreSummary {
        let mut summary = ScoreSummary {
            pending_walking_trips: 0,
            total_walking_trips: 0,
            total_walking_trip_time: Tick::zero(),

            pending_driving_trips: 0,
            total_driving_trips: 0,
            total_driving_trip_time: Tick::zero(),

            completion_time: None,
        };
        // TODO or would it make more sense to aggregate events as they happen?
        for t in &self.trips {
            // Don't count transit
            if t.ped.is_none() {
                continue;
            }
            if t.uses_car {
                if let Some(at) = t.finished_at {
                    summary.total_driving_trip_time += at - t.spawned_at;
                } else {
                    summary.pending_driving_trips += 1;
                    if now >= t.spawned_at {
                        summary.total_driving_trip_time += now - t.spawned_at;
                    }
                }
                summary.total_driving_trips += 1;
            } else {
                if let Some(at) = t.finished_at {
                    summary.total_walking_trip_time += at - t.spawned_at;
                } else {
                    summary.pending_walking_trips += 1;
                    if now >= t.spawned_at {
                        summary.total_walking_trip_time += now - t.spawned_at;
                    }
                }
                summary.total_walking_trips += 1;
            }
        }
        summary
    }

    pub fn active_agents(&self) -> Vec<AgentID> {
        self.active_trip_mode.keys().cloned().collect()
    }

    pub fn trip_to_agent(&self, id: TripID) -> Option<AgentID> {
        let trip = self.trips.get(id.0)?;
        match trip.legs.get(0)? {
            TripLeg::Walk(_) => Some(AgentID::Pedestrian(trip.ped.unwrap())),
            TripLeg::Drive(ref parked, _) => Some(AgentID::Car(parked.car)),
            TripLeg::DriveFromBorder(id, _) => Some(AgentID::Car(*id)),
            TripLeg::Bike(vehicle, _) => Some(AgentID::Car(vehicle.id)),
            // TODO Should be the bus, but apparently transit sim tracks differently?
            TripLeg::RideBus(_, _) => Some(AgentID::Pedestrian(trip.ped.unwrap())),
            TripLeg::ServeBusRoute(id, _) => Some(AgentID::Car(*id)),
        }
    }

    pub fn agent_to_trip(&self, id: AgentID) -> TripID {
        self.active_trip_mode[&id]
    }

    pub fn get_active_trips(&self) -> Vec<TripID> {
        self.active_trip_mode.values().map(|id| *id).collect()
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Trip {
    id: TripID,
    spawned_at: Tick,
    finished_at: Option<Tick>,
    // TODO also uses_bike, so we can track those stats differently too
    uses_car: bool,
    // If none, then this is a bus. The trip will never end.
    ped: Option<PedestrianID>,
    legs: VecDeque<TripLeg>,
}

// Except for Drive (which has to say what car to drive), these don't say where the leg starts.
// That's because it might be unknown -- like when we drive and don't know where we'll wind up
// parking.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum TripLeg {
    Walk(SidewalkSpot),
    // TODO Can maybe collapse Drive and DriveFromBorder by acting like Bike and doing Vehicle,
    // which has CarID
    Drive(ParkedCar, DrivingGoal),
    DriveFromBorder(CarID, DrivingGoal),
    Bike(Vehicle, DrivingGoal),
    RideBus(RouteID, BusStopID),
    ServeBusRoute(CarID, RouteID),
}

impl TripLeg {
    fn uses_car(&self, id: CarID) -> bool {
        match self {
            TripLeg::Drive(parked, _) => parked.car == id,
            TripLeg::DriveFromBorder(car, _) => *car == id,
            TripLeg::Bike(vehicle, _) => vehicle.id == id,
            TripLeg::ServeBusRoute(car, _) => *car == id,
            _ => false,
        }
    }
}

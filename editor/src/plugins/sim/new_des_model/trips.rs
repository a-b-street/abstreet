use crate::plugins::sim::new_des_model::{
    Command, CreateCar, DrivingGoal, ParkingSimState, ParkingSpot, Router, Scheduler, SidewalkSpot,
    Vehicle,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Duration;
use map_model::{BusRouteID, BusStopID, Map, PathRequest, Pathfinder};
use serde_derive::{Deserialize, Serialize};
use sim::{AgentID, CarID, PedestrianID, TripID};
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

    /*
    // Where are we walking next?
    pub fn car_reached_parking_spot(&mut self, car: CarID) -> (TripID, PedestrianID, SidewalkSpot) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];

        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(id, _) => assert_eq!(car, id),
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
    }*/

    pub fn ped_reached_parking_spot(
        &mut self,
        time: Duration,
        ped: PedestrianID,
        spot: ParkingSpot,
        map: &Map,
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self
            .active_trip_mode
            .remove(&AgentID::Pedestrian(ped))
            .unwrap()
            .0];

        assert_eq!(
            trip.legs.pop_front(),
            Some(TripLeg::Walk(SidewalkSpot::parking_spot(
                spot, map, parking
            )))
        );
        let (car, drive_to) = match trip.legs[0] {
            TripLeg::Drive(car, ref to) => (car, to.clone()),
            _ => unreachable!(),
        };
        let parked_car = parking.get_car_at_spot(spot).unwrap();
        assert_eq!(parked_car.vehicle.id, car);

        let path = if let Some(p) = Pathfinder::shortest_distance(
            map,
            PathRequest {
                start: parked_car.get_driving_pos(parking, map),
                end: drive_to.goal_pos(map),
                can_use_bus_lanes: false,
                can_use_bike_lanes: false,
            },
        ) {
            p
        } else {
            println!("Aborting a trip because no path for the car portion!");
            return;
        };

        let router = match drive_to {
            DrivingGoal::ParkNear(b) => Router::park_near(path.convert_to_traversable_list(), b),
            DrivingGoal::Border(_, last_lane) => Router::stop_suddenly(
                path.convert_to_traversable_list(),
                map.get_l(last_lane).length(),
            ),
        };

        scheduler.enqueue_command(Command::SpawnCar(
            time,
            CreateCar::for_parked_car(parked_car, router, trip.id, parking, map),
        ));
    }

    /*pub fn ped_ready_to_bike(&mut self, ped: PedestrianID) -> (TripID, Vehicle, DrivingGoal) {
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

    pub fn ped_reached_building_or_border(&mut self, ped: PedestrianID, now: Duration) {
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
    pub fn car_reached_border(&mut self, car: CarID, now: Duration) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(_, _) => {}
            TripLeg::Bike(_, _) => {}
            x => panic!("Last trip leg {:?} doesn't match car_reached_border", x),
        };
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(now);
    }

    // Combo query/transition from transit
    pub fn should_ped_board_bus(&mut self, ped: PedestrianID, route: BusRouteID) -> bool {
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
    }*/

    // Creation from the interactive part of spawner
    pub fn new_trip(
        &mut self,
        spawned_at: Duration,
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
            uses_car: legs.iter().any(|l| match l {
                TripLeg::Drive(_, _) => true,
                TripLeg::ServeBusRoute(_, _) => true,
                _ => false,
            }),
            legs: VecDeque::from(legs),
        });
        id
    }

    /*
    pub fn active_agents(&self) -> Vec<AgentID> {
        self.active_trip_mode.keys().cloned().collect()
    }

    pub fn trip_to_agent(&self, id: TripID) -> Option<AgentID> {
        let trip = self.trips.get(id.0)?;
        match trip.legs.get(0)? {
            TripLeg::Walk(_) => Some(AgentID::Pedestrian(trip.ped.unwrap())),
            TripLeg::Drive(id, _) => Some(AgentID::Car(*id)),
            TripLeg::Bike(vehicle, _) => Some(AgentID::Car(vehicle.id)),
            // TODO Should be the bus, but apparently transit sim tracks differently?
            TripLeg::RideBus(_, _) => Some(AgentID::Pedestrian(trip.ped.unwrap())),
            TripLeg::ServeBusRoute(id, _) => Some(AgentID::Car(*id)),
        }
    }

    // This will be None for parked cars
    pub fn agent_to_trip(&self, id: AgentID) -> Option<TripID> {
        self.active_trip_mode.get(&id).cloned()
    }

    pub fn get_active_trips(&self) -> Vec<TripID> {
        self.active_trip_mode.values().cloned().collect()
    }

    pub fn tooltip_lines(&self, id: AgentID) -> Vec<String> {
        // Only called for agents that _should_ have trips
        let trip = &self.trips[self.active_trip_mode[&id].0];
        vec![format!(
            "{} has goal {:?}",
            trip.id,
            trip.legs.back().unwrap()
        )]
    }*/
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Trip {
    id: TripID,
    spawned_at: Duration,
    finished_at: Option<Duration>,
    // TODO also uses_bike, so we can track those stats differently too
    uses_car: bool,
    // If none, then this is a bus. The trip will never end.
    ped: Option<PedestrianID>,
    legs: VecDeque<TripLeg>,
}

// These don't specify where the leg starts, since it might be unknown -- like when we drive and
// don't know where we'll wind up parking.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum TripLeg {
    Walk(SidewalkSpot),
    Drive(CarID, DrivingGoal),
    Bike(Vehicle, DrivingGoal),
    RideBus(BusRouteID, BusStopID),
    ServeBusRoute(CarID, BusRouteID),
}

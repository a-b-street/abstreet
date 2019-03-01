use crate::{
    AgentID, CarID, Command, CreateCar, CreatePedestrian, DrivingGoal, Event, ParkingSimState,
    ParkingSpot, PedestrianID, Router, Scheduler, SidewalkPOI, SidewalkSpot, TripID, Vehicle,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Duration;
use map_model::{BuildingID, BusRouteID, BusStopID, IntersectionID, Map, PathRequest, Pathfinder};
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

    events: Vec<Event>,
}

impl TripManager {
    pub fn new() -> TripManager {
        TripManager {
            trips: Vec::new(),
            active_trip_mode: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub fn agent_starting_trip_leg(&mut self, agent: AgentID, trip: TripID) {
        assert!(!self.active_trip_mode.contains_key(&agent));
        // TODO ensure a trip only has one active agent (aka, not walking and driving at the same
        // time)
        self.active_trip_mode.insert(agent, trip);
    }

    pub fn car_reached_parking_spot(
        &mut self,
        time: Duration,
        car: CarID,
        spot: ParkingSpot,
        map: &Map,
        parking: &ParkingSimState,
        scheduler: &mut Scheduler,
    ) {
        self.events.push(Event::CarReachedParkingSpot(car, spot));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];

        match trip.legs.pop_front() {
            Some(TripLeg::Drive(id, DrivingGoal::ParkNear(_))) => assert_eq!(car, id),
            _ => unreachable!(),
        };
        let walk_to = match trip.legs[0] {
            TripLeg::Walk(ref to) => to.clone(),
            _ => unreachable!(),
        };

        let start = SidewalkSpot::parking_spot(spot, map, parking);
        let path = if let Some(p) = Pathfinder::shortest_distance(
            map,
            PathRequest {
                start: start.sidewalk_pos,
                end: walk_to.sidewalk_pos,
                can_use_bus_lanes: false,
                can_use_bike_lanes: false,
            },
        ) {
            p
        } else {
            println!("Aborting a trip because no path for the walking portion!");
            return;
        };

        scheduler.enqueue_command(Command::SpawnPed(
            time,
            CreatePedestrian {
                id: trip.ped.unwrap(),
                start,
                goal: walk_to,
                path,
                trip: trip.id,
            },
        ));
    }

    pub fn ped_reached_parking_spot(
        &mut self,
        time: Duration,
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

        scheduler.enqueue_command(Command::SpawnCar(
            time,
            CreateCar::for_parked_car(
                parked_car,
                drive_to.make_router(path, map),
                trip.id,
                parking,
                map,
            ),
        ));
    }

    pub fn ped_ready_to_bike(
        &mut self,
        time: Duration,
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

        assert_eq!(trip.legs.pop_front(), Some(TripLeg::Walk(spot.clone())));
        let (vehicle, drive_to) = match trip.legs[0] {
            TripLeg::Bike(ref vehicle, ref to) => (vehicle.clone(), to.clone()),
            _ => unreachable!(),
        };
        let driving_pos = match spot.connection {
            SidewalkPOI::BikeRack(p) => p,
            _ => unreachable!(),
        };

        let end = drive_to.goal_pos(map);
        let path = if let Some(p) = Pathfinder::shortest_distance(
            map,
            PathRequest {
                start: driving_pos,
                end,
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
            // TODO Stop closer to the building?
            DrivingGoal::ParkNear(_) => {
                Router::bike_then_stop(path, map.get_l(end.lane()).length() / 2.0)
            }
            DrivingGoal::Border(i, last_lane) => {
                Router::end_at_border(path, map.get_l(last_lane).length(), i)
            }
        };

        scheduler.enqueue_command(Command::SpawnCar(
            time,
            CreateCar::for_appearing(vehicle, driving_pos, router, trip.id),
        ));
    }

    pub fn bike_reached_end(
        &mut self,
        time: Duration,
        bike: CarID,
        bike_rack: SidewalkSpot,
        map: &Map,
        scheduler: &mut Scheduler,
    ) {
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(bike)).unwrap().0];

        match trip.legs.pop_front() {
            Some(TripLeg::Bike(vehicle, DrivingGoal::ParkNear(_))) => assert_eq!(vehicle.id, bike),
            _ => unreachable!(),
        };
        let walk_to = match trip.legs[0] {
            TripLeg::Walk(ref to) => to.clone(),
            _ => unreachable!(),
        };

        let path = if let Some(p) = Pathfinder::shortest_distance(
            map,
            PathRequest {
                start: bike_rack.sidewalk_pos,
                end: walk_to.sidewalk_pos,
                can_use_bus_lanes: false,
                can_use_bike_lanes: false,
            },
        ) {
            p
        } else {
            println!("Aborting a trip because no path for the walking portion!");
            return;
        };

        scheduler.enqueue_command(Command::SpawnPed(
            time,
            CreatePedestrian {
                id: trip.ped.unwrap(),
                start: bike_rack,
                goal: walk_to,
                path,
                trip: trip.id,
            },
        ));
    }

    pub fn ped_reached_building(
        &mut self,
        time: Duration,
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
        assert_eq!(
            trip.legs.pop_front().unwrap(),
            TripLeg::Walk(SidewalkSpot::building(bldg, map))
        );
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(time);
    }

    pub fn ped_reached_border(
        &mut self,
        time: Duration,
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
        assert_eq!(
            trip.legs.pop_front().unwrap(),
            TripLeg::Walk(SidewalkSpot::end_at_border(i, map).unwrap())
        );
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(time);
    }

    pub fn car_or_bike_reached_border(&mut self, time: Duration, car: CarID, i: IntersectionID) {
        self.events.push(Event::CarOrBikeReachedBorder(car, i));
        let trip = &mut self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        match trip.legs.pop_front().unwrap() {
            TripLeg::Drive(_, DrivingGoal::Border(int, _)) => assert_eq!(i, int),
            TripLeg::Bike(_, DrivingGoal::Border(int, _)) => assert_eq!(i, int),
            _ => unreachable!(),
        };
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(time);
    }

    /*
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

    pub fn active_agents(&self) -> Vec<AgentID> {
        self.active_trip_mode.keys().cloned().collect()
    }

    pub fn get_active_trips(&self) -> Vec<TripID> {
        self.active_trip_mode.values().cloned().collect()
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

    pub fn tooltip_lines(&self, id: AgentID) -> Vec<String> {
        // Only called for agents that _should_ have trips
        let trip = &self.trips[self.active_trip_mode[&id].0];
        vec![format!(
            "{} has goal {:?}",
            trip.id,
            trip.legs.back().unwrap()
        )]
    }

    pub fn is_done(&self) -> bool {
        // TODO Buses?
        self.active_trip_mode.is_empty()
    }

    pub fn collect_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }
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

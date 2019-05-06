use crate::{
    AgentID, CarID, Command, CreateCar, CreatePedestrian, DrivingGoal, Event, ParkingSimState,
    ParkingSpot, PedestrianID, Scheduler, SidewalkPOI, SidewalkSpot, TransitSimState, TripID,
    Vehicle, WalkingSimState,
};
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::Duration;
use map_model::{BuildingID, BusRouteID, BusStopID, IntersectionID, Map, PathRequest};
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

    events: Vec<Event>,
}

impl TripManager {
    pub fn new() -> TripManager {
        TripManager {
            trips: Vec::new(),
            active_trip_mode: BTreeMap::new(),
            num_bus_trips: 0,
            events: Vec::new(),
        }
    }

    pub fn new_trip(&mut self, spawned_at: Duration, legs: Vec<TripLeg>) -> TripID {
        assert!(!legs.is_empty());
        // TODO Make sure the legs constitute a valid state machine.

        let id = TripID(self.trips.len());
        self.trips.push(Trip {
            id,
            spawned_at,
            finished_at: None,
            legs: VecDeque::from(legs),
        });
        id
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
            Some(TripLeg::Drive(vehicle, DrivingGoal::ParkNear(_))) => assert_eq!(car, vehicle.id),
            _ => unreachable!(),
        };

        trip.spawn_ped(
            time,
            SidewalkSpot::parking_spot(spot, map, parking),
            map,
            scheduler,
        );
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
            Some(TripLeg::Walk(
                ped,
                SidewalkSpot::parking_spot(spot, map, parking)
            ))
        );
        let (car, drive_to) = match trip.legs[0] {
            TripLeg::Drive(ref vehicle, ref to) => (vehicle.id, to.clone()),
            _ => unreachable!(),
        };
        let parked_car = parking.get_car_at_spot(spot).unwrap();
        assert_eq!(parked_car.vehicle.id, car);

        let start = parked_car.get_driving_pos(parking, map);
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
                "Aborting a trip because no path for the car portion! {:?} to {:?}",
                start, end
            );
            return;
        };

        let router = drive_to.make_router(path, map, parked_car.vehicle.vehicle_type);
        scheduler.push(
            time,
            Command::SpawnCar(CreateCar::for_parked_car(
                parked_car, router, trip.id, parking, map,
            )),
        );
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

        assert_eq!(
            trip.legs.pop_front(),
            Some(TripLeg::Walk(ped, spot.clone()))
        );
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
                "Aborting a trip because no path for the bike portion! {:?} to {:?}",
                driving_pos, end
            );
            return;
        };

        let router = drive_to.make_router(path, map, vehicle.vehicle_type);
        scheduler.push(
            time,
            Command::SpawnCar(CreateCar::for_appearing(
                vehicle,
                driving_pos,
                router,
                trip.id,
            )),
        );
    }

    pub fn bike_reached_end(
        &mut self,
        time: Duration,
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

        trip.spawn_ped(time, bike_rack, map, scheduler);
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
            TripLeg::Walk(ped, SidewalkSpot::building(bldg, map))
        );
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(time);
    }

    // If true, the pedestrian boarded a bus immediately.
    pub fn ped_reached_bus_stop(
        &mut self,
        ped: PedestrianID,
        stop: BusStopID,
        map: &Map,
        transit: &mut TransitSimState,
    ) -> bool {
        self.events.push(Event::PedReachedBusStop(ped, stop));
        let trip = &mut self.trips[self.active_trip_mode[&AgentID::Pedestrian(ped)].0];
        assert_eq!(
            trip.legs[0],
            TripLeg::Walk(ped, SidewalkSpot::bus_stop(stop, map))
        );
        match trip.legs[1] {
            TripLeg::RideBus(_, route, stop2) => {
                if transit.ped_waiting_for_bus(ped, stop, route, stop2) {
                    trip.legs.pop_front();
                    true
                } else {
                    false
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
        time: Duration,
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

        trip.spawn_ped(time, start, map, scheduler);
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
            TripLeg::Walk(ped, SidewalkSpot::end_at_border(i, map).unwrap())
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
            _ => {
                // TODO Should be unreachable
                println!(
                    "Aborting trip {}, because {} couldn't find parking and got stuck",
                    trip.id, car
                );
                return;
            }
        };
        assert!(trip.legs.is_empty());
        assert!(!trip.finished_at.is_some());
        trip.finished_at = Some(time);
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
            TripLeg::Walk(id, _) => Some(AgentID::Pedestrian(*id)),
            TripLeg::Drive(vehicle, _) => Some(AgentID::Car(vehicle.id)),
            // TODO Should be the bus, but apparently transit sim tracks differently?
            TripLeg::RideBus(ped, _, _) => Some(AgentID::Pedestrian(*ped)),
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

    // Not including buses
    pub fn num_active_trips(&self) -> usize {
        self.active_trip_mode.len() - self.num_bus_trips
    }

    pub fn is_done(&self) -> bool {
        self.num_active_trips() == 0
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
    legs: VecDeque<TripLeg>,
}

impl Trip {
    fn is_bus_trip(&self) -> bool {
        self.legs.len() == 1
            && match self.legs[0] {
                TripLeg::ServeBusRoute(_, _) => true,
                _ => false,
            }
    }

    fn spawn_ped(&self, time: Duration, start: SidewalkSpot, map: &Map, scheduler: &mut Scheduler) {
        let (ped, walk_to) = match self.legs[0] {
            TripLeg::Walk(ped, ref to) => (ped, to.clone()),
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
                "Aborting a trip because no path for the walking portion! {:?} to {:?}",
                start, walk_to
            );
            return;
        };

        scheduler.push(
            time,
            Command::SpawnPed(CreatePedestrian {
                id: ped,
                start,
                goal: walk_to,
                path,
                trip: self.id,
            }),
        );
    }
}

// These don't specify where the leg starts, since it might be unknown -- like when we drive and
// don't know where we'll wind up parking.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum TripLeg {
    Walk(PedestrianID, SidewalkSpot),
    Drive(Vehicle, DrivingGoal),
    RideBus(PedestrianID, BusRouteID, BusStopID),
    ServeBusRoute(CarID, BusRouteID),
}

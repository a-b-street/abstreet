use abstutil::{deserialize_btreemap, serialize_btreemap};
use map_model::{BuildingID, BusStop, Map};
use std::collections::{BTreeMap, VecDeque};
use walking::SidewalkSpot;
use {AgentID, CarID, ParkedCar, PedestrianID, RouteID, TripID};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TripManager {
    trips: Vec<Trip>,
    // For quick lookup of active agents
    #[serde(serialize_with = "serialize_btreemap")]
    #[serde(deserialize_with = "deserialize_btreemap")]
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
        (trip.id, trip.ped, walk_to.clone())
    }

    // Where are we driving next?
    pub fn ped_reached_parking_spot(&mut self, ped: PedestrianID) -> (TripID, BuildingID) {
        let trip = &mut self.trips[self.active_trip_mode
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
            TripLeg::Drive(_, ref to) => to,
            ref x => panic!("Next trip leg is {:?}, not walking", x),
        };
        (trip.id, *drive_to)
    }

    // Creation from the interactive part of spawner
    pub fn new_trip(
        &mut self,
        map: &Map,
        ped: PedestrianID,
        start_bldg: BuildingID,
        goal_bldg: BuildingID,
        legs: Vec<TripLeg>,
    ) -> TripID {
        assert!(!legs.is_empty());
        match legs.last().unwrap() {
            TripLeg::Walk(to) => assert_eq!(*to, SidewalkSpot::building(goal_bldg, map)),
            x => panic!(
                "Last leg of trip isn't walking to the goal building; it's {:?}",
                x
            ),
        };

        let id = TripID(self.trips.len());
        self.trips.push(Trip {
            id,
            ped,
            start_bldg,
            goal_bldg,
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
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct Trip {
    id: TripID,
    ped: PedestrianID,
    start_bldg: BuildingID,
    goal_bldg: BuildingID,
    legs: VecDeque<TripLeg>,
}

// Except for Drive (which has to say what car to drive), these don't say where the leg starts.
// That's because it might be unknown -- like when we drive and don't know where we'll wind up
// parking.
#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum TripLeg {
    Walk(SidewalkSpot),
    // Roads might be long -- what building do we ultimately want to park near?
    Drive(ParkedCar, BuildingID),
    RideBus(RouteID, BusStop),
}

impl TripLeg {
    fn uses_car(&self, id: CarID) -> bool {
        match self {
            TripLeg::Drive(parked, _) => parked.car == id,
            _ => false,
        }
    }
}

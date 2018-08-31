use map_model::BuildingID;
use std::collections::BTreeMap;
use {AgentID, CarID, PedestrianID, TripID};

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct TripManager {
    trips: Vec<Trip>,
    // For quick lookup of active agents
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

    pub fn car_reached_parking_spot(&mut self, car: CarID) -> (TripID, PedestrianID, BuildingID) {
        let trip = &self.trips[self.active_trip_mode.remove(&AgentID::Car(car)).unwrap().0];
        (trip.id, trip.ped, trip.goal_bldg)
    }

    pub fn ped_reached_parking_spot(&mut self, ped: PedestrianID) -> (TripID, BuildingID) {
        let trip = &self.trips[self.active_trip_mode
                                   .remove(&AgentID::Pedestrian(ped))
                                   .unwrap()
                                   .0];
        (trip.id, trip.goal_bldg)
    }

    // Creation from the interactive part of spawner
    pub fn new_trip(
        &mut self,
        ped: PedestrianID,
        start_bldg: BuildingID,
        use_car: Option<CarID>,
        goal_bldg: BuildingID,
    ) -> TripID {
        let id = TripID(self.trips.len());
        self.trips.push(Trip {
            id,
            ped,
            start_bldg,
            use_car,
            goal_bldg,
        });
        id
    }

    // Query from spawner
    pub fn get_trip_using_car(&self, car: CarID) -> Option<TripID> {
        self.trips
            .iter()
            .find(|t| t.use_car == Some(car))
            .map(|t| t.id)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
struct Trip {
    id: TripID,
    ped: PedestrianID,
    start_bldg: BuildingID,
    // Later, this could be an enum of mode choices, or something even more complicated
    use_car: Option<CarID>,
    goal_bldg: BuildingID,
}

use crate::{AgentID, CarID, ParkingSpot, PedestrianID, PersonID, TripID, TripMode};
use geom::Duration;
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, LaneID, Map, Path, PathRequest, Traversable,
};
use serde_derive::{Deserialize, Serialize};

// Many of these were created for a test framework that's been abandoned. They could be removed or
// have their API adjusted, but it's not urgent; publishing an event that's not used by Analytics
// has no performance impact.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Event {
    CarReachedParkingSpot(CarID, ParkingSpot),
    CarLeftParkingSpot(CarID, ParkingSpot),
    CarOrBikeReachedBorder(CarID, IntersectionID),

    BusArrivedAtStop(CarID, BusRouteID, BusStopID),
    BusDepartedFromStop(CarID, BusRouteID, BusStopID),

    PersonEntersBuilding(PersonID, BuildingID),
    PersonLeavesBuilding(PersonID, BuildingID),

    PedReachedParkingSpot(PedestrianID, ParkingSpot),
    PedReachedBorder(PedestrianID, IntersectionID),

    BikeStoppedAtSidewalk(CarID, LaneID),

    AgentEntersTraversable(AgentID, Traversable),
    IntersectionDelayMeasured(IntersectionID, Duration),

    TripFinished {
        trip: TripID,
        mode: TripMode,
        total_time: Duration,
        blocked_time: Duration,
    },
    TripAborted(TripID, TripMode),
    TripPhaseStarting(
        TripID,
        PersonID,
        TripMode,
        Option<PathRequest>,
        TripPhaseType,
    ),

    // Just use for parking replanning. Not happy about copying the full path in here, but the way
    // to plumb info into Analytics is Event.
    PathAmended(Path),

    Alert(IntersectionID, String),
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum TripPhaseType {
    Driving,
    Walking,
    Biking,
    Parking,
    WaitingForBus(BusRouteID, BusStopID),
    // What stop did they board at?
    RidingBus(BusRouteID, BusStopID, CarID),
    Aborted,
    Finished,
    DelayedStart,
}

impl TripPhaseType {
    pub fn describe(self, map: &Map) -> String {
        match self {
            TripPhaseType::Driving => "driving".to_string(),
            TripPhaseType::Walking => "walking".to_string(),
            TripPhaseType::Biking => "biking".to_string(),
            TripPhaseType::Parking => "parking".to_string(),
            TripPhaseType::WaitingForBus(r, _) => format!("waiting for bus {}", map.get_br(r).name),
            TripPhaseType::RidingBus(r, _, _) => format!("riding bus {}", map.get_br(r).name),
            TripPhaseType::Aborted => "trip aborted due to some bug".to_string(),
            TripPhaseType::Finished => "trip finished".to_string(),
            TripPhaseType::DelayedStart => "delayed by previous trip taking too long".to_string(),
        }
    }
}

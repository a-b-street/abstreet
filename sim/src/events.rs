use serde::{Deserialize, Serialize};

use geom::{Duration, Speed};
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, LaneID, Map, Path, PathRequest, Traversable,
    TurnID,
};

use crate::{AgentID, CarID, ParkingSpot, PedestrianID, PersonID, TripID, TripMode};

/// As a simulation runs, different systems emit Events. This cleanly separates the internal
/// mechanics of the simulation from consumers that just want to know what's happening.
///
/// An Event always occurs at a particular time, plumbed separately to consumers.
///
/// Many of these were created for a test framework that's been abandoned. They could be removed or
/// have their API adjusted, but it's not urgent; publishing an event that's not used by Analytics
/// has no performance impact.
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Event {
    CarReachedParkingSpot(CarID, ParkingSpot),
    CarLeftParkingSpot(CarID, ParkingSpot),

    BusArrivedAtStop(CarID, BusRouteID, BusStopID),
    BusDepartedFromStop(CarID, BusRouteID, BusStopID),
    /// How long waiting at the stop?
    PassengerBoardsTransit(PersonID, CarID, BusRouteID, BusStopID, Duration),
    PassengerAlightsTransit(PersonID, CarID, BusRouteID, BusStopID),

    PersonEntersBuilding(PersonID, BuildingID),
    PersonLeavesBuilding(PersonID, BuildingID),
    /// None if cancelled
    PersonLeavesMap(PersonID, Option<AgentID>, IntersectionID),
    PersonEntersMap(PersonID, AgentID, IntersectionID),

    PedReachedParkingSpot(PedestrianID, ParkingSpot),

    BikeStoppedAtSidewalk(CarID, LaneID),

    /// If the agent is a transit vehicle, then include a count of how many passengers are on
    /// board.
    AgentEntersTraversable(AgentID, Option<TripID>, Traversable, Option<usize>),
    /// TripID, TurnID (Where the delay was encountered), Time spent waiting at that turn
    IntersectionDelayMeasured(TripID, TurnID, AgentID, Duration),

    TripFinished {
        trip: TripID,
        mode: TripMode,
        total_time: Duration,
        blocked_time: Duration,
    },
    TripCancelled(TripID, TripMode),
    TripPhaseStarting(TripID, PersonID, Option<PathRequest>, TripPhaseType),
    /// TripID, LaneID (Where the delay was encountered), Average Speed, Max Speed
    LaneSpeedPercentage(TripID, LaneID, Speed, Speed),

    /// Just use for parking replanning. Not happy about copying the full path in here, but the way
    /// to plumb info into Analytics is Event.
    PathAmended(Path),

    Alert(AlertLocation, String),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum AlertLocation {
    Nil,
    Intersection(IntersectionID),
    Person(PersonID),
    Building(BuildingID),
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub enum TripPhaseType {
    Driving,
    Walking,
    Biking,
    Parking,
    WaitingForBus(BusRouteID, BusStopID),
    /// What stop did they board at?
    RidingBus(BusRouteID, BusStopID, CarID),
    Cancelled,
    Finished,
    DelayedStart,
}

impl TripPhaseType {
    pub fn describe(self, map: &Map) -> String {
        match self {
            TripPhaseType::Driving => "Driving".to_string(),
            TripPhaseType::Walking => "Walking".to_string(),
            TripPhaseType::Biking => "Biking".to_string(),
            TripPhaseType::Parking => "Parking".to_string(),
            TripPhaseType::WaitingForBus(r, _) => {
                format!("Waiting for bus {}", map.get_br(r).full_name)
            }
            TripPhaseType::RidingBus(r, _, _) => format!("Riding bus {}", map.get_br(r).full_name),
            TripPhaseType::Cancelled => "Trip was cancelled due to some bug".to_string(),
            TripPhaseType::Finished => "Trip finished".to_string(),
            TripPhaseType::DelayedStart => "Delayed by a previous trip taking too long".to_string(),
        }
    }
}

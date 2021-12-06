use serde::{Deserialize, Serialize};

use geom::Duration;
use map_model::{
    BuildingID, IntersectionID, LaneID, Map, Path, PathRequest, TransitRouteID, TransitStopID,
    Traversable, TurnID,
};

use crate::{AgentID, CarID, ParkingSpot, PedestrianID, PersonID, Problem, TripID, TripMode};

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

    BusArrivedAtStop(CarID, TransitRouteID, TransitStopID),
    BusDepartedFromStop(CarID, TransitRouteID, TransitStopID),
    /// How long waiting at the stop?
    PassengerBoardsTransit(PersonID, CarID, TransitRouteID, TransitStopID, Duration),
    PassengerAlightsTransit(PersonID, CarID, TransitRouteID, TransitStopID),

    PersonEntersBuilding(PersonID, BuildingID),
    PersonLeavesBuilding(PersonID, BuildingID),
    /// None if cancelled
    PersonLeavesMap(PersonID, Option<AgentID>, IntersectionID),
    PersonEntersMap(PersonID, AgentID, IntersectionID),

    PedReachedParkingSpot(PedestrianID, ParkingSpot),

    BikeStoppedAtSidewalk(CarID, LaneID),

    ProblemEncountered(TripID, Problem),

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
    WaitingForBus(TransitRouteID, TransitStopID),
    /// What stop did they board at?
    RidingBus(TransitRouteID, TransitStopID, CarID),
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
                format!("Waiting for transit route {}", map.get_tr(r).long_name)
            }
            TripPhaseType::RidingBus(r, _, _) => {
                format!("Riding route {}", map.get_tr(r).long_name)
            }
            TripPhaseType::Cancelled => "Trip was cancelled due to some bug".to_string(),
            TripPhaseType::Finished => "Trip finished".to_string(),
            TripPhaseType::DelayedStart => "Delayed by a previous trip taking too long".to_string(),
        }
    }
}

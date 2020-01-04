use crate::{AgentID, CarID, ParkingSpot, PedestrianID, TripID, TripMode};
use geom::Duration;
use map_model::{
    BuildingID, BusRouteID, BusStopID, IntersectionID, LaneID, Path, PathRequest, Traversable,
};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Event {
    CarReachedParkingSpot(CarID, ParkingSpot),
    CarOrBikeReachedBorder(CarID, IntersectionID),

    BusArrivedAtStop(CarID, BusRouteID, BusStopID),
    BusDepartedFromStop(CarID, BusRouteID, BusStopID),

    PedReachedParkingSpot(PedestrianID, ParkingSpot),
    PedReachedBuilding(PedestrianID, BuildingID),
    PedReachedBorder(PedestrianID, IntersectionID),
    PedReachedBusStop(PedestrianID, BusStopID, BusRouteID),
    PedEntersBus(PedestrianID, CarID, BusRouteID),
    PedLeavesBus(PedestrianID, CarID, BusRouteID),

    BikeStoppedAtSidewalk(CarID, LaneID),

    AgentEntersTraversable(AgentID, Traversable),
    IntersectionDelayMeasured(IntersectionID, Duration),

    TripFinished(TripID, TripMode, Duration),
    TripAborted(TripID),
    TripPhaseStarting(TripID, Option<PathRequest>, String),

    // Just use for parking replanning. Not happy about copying the full path in here, but the way
    // to plumb info into Analytics is Event.
    PathAmended(Path),
}

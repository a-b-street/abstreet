use crate::{AgentID, CarID, ParkingSpot, PedestrianID};
use map_model::{BuildingID, BusRouteID, BusStopID, IntersectionID, LaneID, Traversable};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    CarReachedParkingSpot(CarID, ParkingSpot),
    CarOrBikeReachedBorder(CarID, IntersectionID),

    BusArrivedAtStop(CarID, BusRouteID, BusStopID),
    BusDepartedFromStop(CarID, BusRouteID, BusStopID),

    PedReachedParkingSpot(PedestrianID, ParkingSpot),
    PedReachedBuilding(PedestrianID, BuildingID),
    PedReachedBorder(PedestrianID, IntersectionID),
    PedReachedBusStop(PedestrianID, BusStopID),
    PedEntersBus(PedestrianID, CarID),
    PedLeavesBus(PedestrianID, CarID),

    BikeStoppedAtSidewalk(CarID, LaneID),

    AgentEntersTraversable(AgentID, Traversable),
}

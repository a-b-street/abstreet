use crate::{AgentID, CarID, ParkingSpot, PedestrianID};
use map_model::{BuildingID, BusStopID, IntersectionID, Traversable};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    CarReachedParkingSpot(CarID, ParkingSpot),
    CarOrBikeReachedBorder(CarID, IntersectionID),

    BusArrivedAtStop(CarID, BusStopID),
    BusDepartedFromStop(CarID, BusStopID),

    PedReachedParkingSpot(PedestrianID, ParkingSpot),
    PedReachedBuilding(PedestrianID, BuildingID),
    PedReachedBorder(PedestrianID, IntersectionID),
    PedReachedBusStop(PedestrianID, BusStopID),
    PedEntersBus(PedestrianID, CarID),
    PedLeavesBus(PedestrianID, CarID),

    // TODO Remove this one
    AgentEntersTraversable(AgentID, Traversable),
}

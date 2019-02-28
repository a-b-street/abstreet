use crate::{AgentID, CarID, ParkingSpot, PedestrianID};
use map_model::{BuildingID, BusStopID, Traversable};
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    CarReachedParkingSpot(CarID, ParkingSpot),

    BusArrivedAtStop(CarID, BusStopID),
    BusDepartedFromStop(CarID, BusStopID),

    PedReachedParkingSpot(PedestrianID, ParkingSpot),
    PedReachedBuilding(PedestrianID, BuildingID),
    PedReachedBusStop(PedestrianID, BusStopID),
    PedEntersBus(PedestrianID, CarID),
    PedLeavesBus(PedestrianID, CarID),

    AgentEntersTraversable(AgentID, Traversable),
}

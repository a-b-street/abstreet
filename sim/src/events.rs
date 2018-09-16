use intersections::Request;
use map_model::{BuildingID, BusStopID};
use {AgentID, CarID, On, ParkedCar, ParkingSpot, PedestrianID};

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    // TODO CarFinishedParking
    // TODO and the pedestrian / trip associated with it?
    CarReachedParkingSpot(ParkedCar),
    // TODO and the car / trip?
    PedReachedParkingSpot(PedestrianID, ParkingSpot),
    // TODO CarFinishedUnparking
    BusArrivedAtStop(CarID, BusStopID),
    BusDepartedFromStop(CarID, BusStopID),

    PedReachedBuilding(PedestrianID, BuildingID),
    PedReachedBusStop(PedestrianID, BusStopID),
    PedEntersBus(PedestrianID, CarID),
    PedLeavesBus(PedestrianID, CarID),

    // TODO split up into cases or not?
    AgentEntersTraversable(AgentID, On),
    AgentLeavesTraversable(AgentID, On),

    // TODO maybe AgentRequestsTurn?
    IntersectionAcceptsRequest(Request),
}

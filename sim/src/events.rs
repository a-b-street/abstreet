use intersections::Request;
use map_model::{BuildingID, BusStop};
use {AgentID, CarID, On, ParkedCar, ParkingSpot, PedestrianID};

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    // TODO CarFinishedParking
    // TODO and the pedestrian / trip associated with it?
    CarReachedParkingSpot(ParkedCar),
    // TODO and the car / trip?
    PedReachedParkingSpot(PedestrianID, ParkingSpot),
    // TODO CarFinishedUnparking
    BusArrivedAtStop(CarID, BusStop),
    BusDepartedFromStop(CarID, BusStop),

    PedReachedBuilding(PedestrianID, BuildingID),

    // TODO split up into cases or not?
    AgentEntersTraversable(AgentID, On),
    AgentLeavesTraversable(AgentID, On),

    // TODO maybe AgentRequestsTurn?
    IntersectionAcceptsRequest(Request),
}

use kml::ExtraShapeID;
use map_model::{BuildingID, IntersectionID, LaneID, ParcelID, TurnID};
use sim::{CarID, PedestrianID};

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug)]
pub enum ID {
    Lane(LaneID),
    Intersection(IntersectionID),
    Turn(TurnID),
    Building(BuildingID),
    Car(CarID),
    Pedestrian(PedestrianID),
    ExtraShape(ExtraShapeID),
    Parcel(ParcelID),
}

use colors::ColorScheme;
use control::ControlMap;
use ezgui::Canvas;
use kml::ExtraShapeID;
use map_model::{BuildingID, BusStopID, IntersectionID, LaneID, Map, ParcelID, TurnID};
use sim::{CarID, PedestrianID, Sim};

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
    BusStop(BusStopID),
}

// For plugins and rendering. Not sure what module this should live in, here seems fine.
pub struct Ctx<'a> {
    pub cs: &'a ColorScheme,
    pub map: &'a Map,
    pub control_map: &'a ControlMap,
    pub canvas: &'a Canvas,
    pub sim: &'a Sim,
}

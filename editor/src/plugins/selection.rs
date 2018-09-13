use colors::{ColorScheme, Colors};
use graphics::types::Color;
use kml::ExtraShapeID;
use map_model::{BuildingID, IntersectionID, LaneID, TurnID};
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
    //Parcel(ParcelID),
}

#[derive(Clone)]
pub enum SelectionState {
    Empty,
    Selected(ID),
}

impl SelectionState {
    pub fn handle_mouseover(&self, maybe_id: Option<ID>) -> SelectionState {
        if let Some(some_id) = maybe_id {
            SelectionState::Selected(some_id)
        } else {
            SelectionState::Empty
        }
    }

    pub fn color_for(&self, id: ID, cs: &ColorScheme) -> Option<Color> {
        let selected = match self {
            SelectionState::Selected(x) => *x == id,
            _ => false,
        };
        if selected {
            Some(cs.get(Colors::Selected))
        } else {
            None
        }
    }
}

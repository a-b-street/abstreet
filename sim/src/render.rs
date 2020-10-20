//! Intermediate structures so that sim and game crates don't have a cyclic dependency.

use geom::{Angle, Distance, PolyLine, Pt2D};
use map_model::{BuildingID, ParkingLotID, Traversable, TurnID};

use crate::{AgentID, CarID, PedestrianID, PersonID};

#[derive(Clone)]
pub struct DrawPedestrianInput {
    pub id: PedestrianID,
    pub pos: Pt2D,
    pub facing: Angle,
    pub waiting_for_turn: Option<TurnID>,
    pub preparing_bike: bool,
    pub waiting_for_bus: bool,
    pub on: Traversable,
}

pub struct DrawPedCrowdInput {
    pub low: Distance,
    pub high: Distance,
    pub members: Vec<PedestrianID>,
    pub location: PedCrowdLocation,
}

#[derive(Clone)]
pub enum PedCrowdLocation {
    /// bool is contraflow
    Sidewalk(Traversable, bool),
    BldgDriveway(BuildingID),
    LotDriveway(ParkingLotID),
}

#[derive(Clone)]
pub struct DrawCarInput {
    pub id: CarID,
    pub waiting_for_turn: Option<TurnID>,
    pub status: CarStatus,
    pub show_parking_intent: bool,
    /// Front of the car
    pub on: Traversable,
    /// Possibly the rest
    pub partly_on: Vec<Traversable>,
    pub label: Option<String>,

    // Starts at the BACK of the car.
    pub body: PolyLine,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarStatus {
    Moving,
    Parked,
}

pub struct UnzoomedAgent {
    pub id: AgentID,
    pub pos: Pt2D,
    /// None means a bus.
    pub person: Option<PersonID>,
    /// True only for cars currently looking for parking. I don't want this struct to grow, but
    /// this is important enough to call out here.
    pub parking: bool,
}

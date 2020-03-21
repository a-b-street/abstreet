use crate::{CarID, PedestrianID, VehicleType};
use geom::{Angle, Distance, PolyLine, Pt2D, Time};
use map_model::{BuildingID, Map, Traversable, TurnID};

// Intermediate structures so that sim and game crates don't have a cyclic dependency.
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
    // bool is contraflow
    Sidewalk(Traversable, bool),
    FrontPath(BuildingID),
}

#[derive(Clone)]
pub struct DrawCarInput {
    pub id: CarID,
    pub waiting_for_turn: Option<TurnID>,
    pub status: CarStatus,
    pub on: Traversable,
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
    // None means a pedestrian.
    pub vehicle_type: Option<VehicleType>,
    pub pos: Pt2D,
}

// TODO Can we return borrows instead? Nice for time travel, not for main sim?
// actually good for main sim too; we're constantly calculating stuff while sim is paused
// otherwise? except we don't know what to calculate. maybe cache it?
pub trait GetDrawAgents {
    fn time(&self) -> Time;
    // Every time the time changes, this should increase. For smoothly animating stuff.
    fn step_count(&self) -> usize;
    fn get_draw_car(&self, id: CarID, map: &Map) -> Option<DrawCarInput>;
    fn get_draw_ped(&self, id: PedestrianID, map: &Map) -> Option<DrawPedestrianInput>;
    fn get_draw_cars(&self, on: Traversable, map: &Map) -> Vec<DrawCarInput>;
    fn get_draw_peds(
        &self,
        on: Traversable,
        map: &Map,
    ) -> (Vec<DrawPedestrianInput>, Vec<DrawPedCrowdInput>);
    fn get_all_draw_cars(&self, map: &Map) -> Vec<DrawCarInput>;
    fn get_all_draw_peds(&self, map: &Map) -> Vec<DrawPedestrianInput>;
    fn get_unzoomed_agents(&self, map: &Map) -> Vec<UnzoomedAgent>;
}

pub struct DontDrawAgents;

impl GetDrawAgents for DontDrawAgents {
    fn time(&self) -> Time {
        Time::START_OF_DAY
    }
    fn step_count(&self) -> usize {
        0
    }
    fn get_draw_car(&self, _: CarID, _: &Map) -> Option<DrawCarInput> {
        None
    }
    fn get_draw_ped(&self, _: PedestrianID, _: &Map) -> Option<DrawPedestrianInput> {
        None
    }
    fn get_draw_cars(&self, _: Traversable, _: &Map) -> Vec<DrawCarInput> {
        Vec::new()
    }
    fn get_draw_peds(
        &self,
        _: Traversable,
        _: &Map,
    ) -> (Vec<DrawPedestrianInput>, Vec<DrawPedCrowdInput>) {
        (Vec::new(), Vec::new())
    }
    fn get_all_draw_cars(&self, _: &Map) -> Vec<DrawCarInput> {
        Vec::new()
    }
    fn get_all_draw_peds(&self, _: &Map) -> Vec<DrawPedestrianInput> {
        Vec::new()
    }
    fn get_unzoomed_agents(&self, _: &Map) -> Vec<UnzoomedAgent> {
        Vec::new()
    }
}

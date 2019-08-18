use crate::{CarID, PedestrianID, VehicleType};
use geom::{Angle, Distance, Duration, PolyLine, Pt2D};
use map_model::{Map, Traversable, TurnID};

// Intermediate structures so that sim and editor crates don't have a cyclic dependency.
#[derive(Clone)]
pub struct DrawPedestrianInput {
    pub id: PedestrianID,
    pub pos: Pt2D,
    pub facing: Angle,
    pub waiting_for_turn: Option<TurnID>,
    pub preparing_bike: bool,
    pub on: Traversable,
}

pub struct DrawPedCrowdInput {
    pub low: Distance,
    pub high: Distance,
    pub contraflow: bool,
    pub members: Vec<PedestrianID>,
    pub on: Traversable,
}

#[derive(Clone)]
pub struct DrawCarInput {
    pub id: CarID,
    pub waiting_for_turn: Option<TurnID>,
    pub status: CarStatus,
    pub on: Traversable,
    pub label: Option<String>,

    // Starts at the BACK of the car. TODO Dedupe unused old stuff.
    pub body: PolyLine,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CarStatus {
    Moving,
    Stuck,
    Parked,
    Debug,
}

pub struct UnzoomedAgent {
    // None means a pedestrian.
    pub vehicle_type: Option<VehicleType>,
    pub pos: Pt2D,
    pub time_spent_blocked: Duration,
    pub percent_dist_crossed: f64,
}

// TODO Can we return borrows instead? Nice for time travel, not for main sim?
// actually good for main sim too; we're constantly calculating stuff while sim is paused
// otherwise? except we don't know what to calculate. maybe cache it?
pub trait GetDrawAgents {
    fn time(&self) -> Duration;
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
}

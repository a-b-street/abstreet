use dimensioned::si;
use draw_car::DrawCar;
use intersections::IntersectionSimState;
use map_model::{LaneID, Map, TurnID};
use std;
use std::collections::VecDeque;
use {CarID, CarState, On, Tick};
use erased_serde;

// At all speeds (including at rest), cars must be at least this far apart.
pub const FOLLOWING_DISTANCE: si::Meter<f64> = si::Meter {
    value_unsafe: 8.0,
    _marker: std::marker::PhantomData,
};

// These might have slightly different meanings in different models...
pub(crate) enum Action {
    Vanish,      // done with route (and transitioning to a different state isn't implemented yet)
    Continue,    // need more time to cross the current spot
    Goto(On),    // go somewhere
    WaitFor(On), // ready to go somewhere, but can't yet for some reason
}

pub(crate) fn choose_turn(path: &VecDeque<LaneID>, waiting_for: &Option<On>, from: LaneID, map: &Map) -> TurnID {
    assert!(waiting_for.is_none());
    for t in map.get_turns_from_lane(from) {
        if t.dst == path[0] {
            return t.id;
        }
    }
    panic!("No turn from {} to {}", from, path[0]);
}

// TODO some of DrivingSimState could maybe be parameterized

pub trait DrivingSim: erased_serde::Serialize {
    fn get_car_state(&self, c: CarID) -> CarState;
    fn get_active_and_waiting_count(&self) -> (usize, usize);
    fn tooltip_lines(&self, id: CarID) -> Option<Vec<String>>;

    fn toggle_debug(&mut self, id: CarID);

    fn edit_remove_lane(&mut self, id: LaneID);
    fn edit_add_lane(&mut self, id: LaneID);
    fn edit_remove_turn(&mut self, id: TurnID);
    fn edit_add_turn(&mut self, id: TurnID, map: &Map);

    fn step(&mut self, time: Tick, map: &Map, intersections: &mut IntersectionSimState);

    fn start_car_on_lane(
        &mut self,
        time: Tick,
        car: CarID,
        path: VecDeque<LaneID>,
    ) -> bool;
    fn get_empty_lanes(&self, map: &Map) -> Vec<LaneID>;

    fn get_draw_car(&self, id: CarID, time: Tick, map: &Map) -> Option<DrawCar>;
    fn get_draw_cars_on_lane(&self, lane: LaneID, time: Tick, map: &Map) -> Vec<DrawCar>;
    fn get_draw_cars_on_turn(&self, turn: TurnID, time: Tick, map: &Map) -> Vec<DrawCar>;
}

serialize_trait_object!(DrivingSim);

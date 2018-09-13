// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use control::ControlMap;
use ezgui::{Canvas, GfxCtx, UserInput};
use graphics::types::Color;
use kml::ExtraShapeID;
use map_model::{BuildingID, IntersectionID, LaneID, Map, TurnID};
use piston::input::{Button, Key, ReleaseEvent};
use render::{DrawMap, Renderable};
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
    //Parcel(ParcelID),
}

// TODO maybe move to Renderable?
impl ID {
    fn debug(&self, map: &Map, control_map: &ControlMap, sim: &mut Sim) {
        match self {
            ID::Lane(id) => {
                map.get_l(*id).dump_debug();
            }
            ID::Intersection(id) => {
                map.get_i(*id).dump_debug();
                sim.debug_intersection(*id, control_map);
            }
            ID::Turn(_) => {}
            ID::Building(id) => {
                map.get_b(*id).dump_debug();
            }
            ID::Car(_) => {}
            ID::Pedestrian(id) => {
                sim.debug_ped(*id);
            }
            ID::ExtraShape(_) => {}
        }
    }

    fn tooltip_lines(&self, map: &Map, draw_map: &DrawMap, sim: &Sim) -> Vec<String> {
        match self {
            ID::Lane(id) => draw_map.get_l(*id).tooltip_lines(map),
            ID::Building(id) => draw_map.get_b(*id).tooltip_lines(map),
            ID::Car(id) => sim.car_tooltip(*id),
            ID::Pedestrian(id) => sim.ped_tooltip(*id),
            ID::Intersection(id) => vec![format!("{}", id)],
            ID::Turn(id) => map.get_t(*id).tooltip_lines(map),
            ID::ExtraShape(id) => draw_map.get_es(*id).tooltip_lines(map),
        }
    }
}

#[derive(Clone)]
pub enum SelectionState {
    Empty,
    Selected(ID),
    Tooltip(ID),
}

impl SelectionState {
    // TODO shouldnt these two consume self?
    pub fn handle_mouseover(&self, maybe_id: Option<ID>) -> SelectionState {
        if let Some(some_id) = maybe_id {
            // Don't break out of the tooltip state
            if let SelectionState::Tooltip(_) = *self {
                SelectionState::Tooltip(some_id)
            } else {
                SelectionState::Selected(some_id)
            }
        } else {
            SelectionState::Empty
        }
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        sim: &mut Sim,
        control_map: &ControlMap,
    ) -> bool {
        let mut new_state: Option<SelectionState> = None;
        let active = match self {
            SelectionState::Empty => false,
            SelectionState::Selected(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {:?}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(*id));
                    true
                } else if input.key_pressed(Key::D, "debug") {
                    id.debug(map, control_map, sim);
                    true
                } else {
                    false
                }
            }
            SelectionState::Tooltip(id) => {
                if let Some(Button::Keyboard(Key::LCtrl)) =
                    input.use_event_directly().release_args()
                {
                    new_state = Some(SelectionState::Selected(*id));
                    true
                } else {
                    false
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        active
    }

    pub fn draw(&self, map: &Map, canvas: &Canvas, draw_map: &DrawMap, sim: &Sim, g: &mut GfxCtx) {
        match *self {
            SelectionState::Empty => {}
            SelectionState::Selected(_) => {}
            SelectionState::Tooltip(some_id) => {
                canvas.draw_mouse_tooltip(g, &some_id.tooltip_lines(map, draw_map, sim));
            }
        }
    }

    pub fn color_for(&self, id: ID, cs: &ColorScheme) -> Option<Color> {
        let selected = match self {
            SelectionState::Selected(x) => *x == id,
            SelectionState::Tooltip(x) => *x == id,
            _ => false,
        };
        if selected {
            Some(cs.get(Colors::Selected))
        } else {
            None
        }
    }
}

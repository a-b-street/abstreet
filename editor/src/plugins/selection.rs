// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use control::ControlMap;
use ezgui::{Canvas, GfxCtx, UserInput};
use graphics::types::Color;
use kml::ExtraShapeID;
use map_model::{BuildingID, IntersectionID, LaneID, Map, TurnID};
use piston::input::{Button, Key, ReleaseEvent};
use render;
use render::Renderable;
use sim::{CarID, PedestrianID, Sim};
use std::collections::HashSet;

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

// TODO lots of code duplication here, but haven't quite worked out how to improve it...

#[derive(Clone)]
pub enum SelectionState {
    Empty,
    SelectedIntersection(IntersectionID),
    SelectedLane(LaneID),
    SelectedBuilding(BuildingID),
    SelectedTurn(TurnID),
    SelectedCar(CarID),
    SelectedPedestrian(PedestrianID),
    SelectedExtraShape(ExtraShapeID),
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
                selection_state_for(some_id)
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
            SelectionState::SelectedLane(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Lane(*id)));
                    true
                } else if input.key_pressed(Key::D, "debug") {
                    map.get_l(*id).dump_debug();
                    true
                } else {
                    false
                }
            }
            SelectionState::Tooltip(id) => {
                if let Some(Button::Keyboard(Key::LCtrl)) =
                    input.use_event_directly().release_args()
                {
                    new_state = Some(selection_state_for(*id));
                    true
                } else {
                    false
                }
            }
            SelectionState::SelectedPedestrian(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Pedestrian(*id)));
                    true
                } else if input.key_pressed(Key::D, "debug") {
                    sim.debug_ped(*id);
                    true
                } else {
                    false
                }
            }
            SelectionState::SelectedIntersection(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Intersection(*id)));
                    true
                } else if input.key_pressed(Key::D, "debug") {
                    map.get_i(*id).dump_debug();
                    sim.debug_intersection(*id, control_map);
                    true
                } else {
                    false
                }
            }
            SelectionState::SelectedBuilding(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Building(*id)));
                    true
                } else if input.key_pressed(Key::D, "debug") {
                    map.get_b(*id).dump_debug();
                    true
                } else {
                    false
                }
            }
            SelectionState::SelectedTurn(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Turn(*id)));
                    true
                } else {
                    false
                }
            }
            SelectionState::SelectedCar(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Car(*id)));
                    true
                } else {
                    false
                }
            }
            SelectionState::SelectedExtraShape(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::ExtraShape(*id)));
                    true
                } else {
                    false
                }
            }
            SelectionState::Empty => false,
        };
        if let Some(s) = new_state {
            *self = s;
        }
        active
    }

    pub fn draw(
        &self,
        map: &Map,
        canvas: &Canvas,
        draw_map: &render::DrawMap,
        sim: &Sim,
        g: &mut GfxCtx,
    ) {
        match *self {
            SelectionState::Empty
            | SelectionState::SelectedTurn(_)
            | SelectionState::SelectedBuilding(_)
            | SelectionState::SelectedCar(_)
            | SelectionState::SelectedPedestrian(_)
            | SelectionState::SelectedExtraShape(_)
            | SelectionState::SelectedLane(_)
            | SelectionState::SelectedIntersection(_) => {}
            SelectionState::Tooltip(some_id) => {
                let lines = match some_id {
                    ID::Lane(id) => draw_map.get_l(id).tooltip_lines(map),
                    ID::Building(id) => draw_map.get_b(id).tooltip_lines(map),
                    ID::Car(id) => sim.car_tooltip(id),
                    ID::Pedestrian(id) => sim.ped_tooltip(id),
                    ID::Intersection(id) => vec![format!("{}", id)],
                    ID::Turn(id) => map.get_t(id).tooltip_lines(map),
                    ID::ExtraShape(id) => draw_map.get_es(id).tooltip_lines(map),
                };
                canvas.draw_mouse_tooltip(g, &lines);
            }
        }
    }

    pub fn color_for(&self, id: ID, cs: &ColorScheme) -> Option<Color> {
        let selected = match (self, id) {
            (SelectionState::SelectedIntersection(x), ID::Intersection(y)) => *x == y,
            (SelectionState::SelectedLane(x), ID::Lane(y)) => *x == y,
            (SelectionState::SelectedBuilding(x), ID::Building(y)) => *x == y,
            (SelectionState::SelectedTurn(x), ID::Turn(y)) => *x == y,
            (SelectionState::SelectedCar(x), ID::Car(y)) => *x == y,
            (SelectionState::SelectedPedestrian(x), ID::Pedestrian(y)) => *x == y,
            (SelectionState::SelectedExtraShape(x), ID::ExtraShape(y)) => *x == y,
            (SelectionState::Tooltip(x), y) => *x == y,
            _ => false,
        };
        if selected {
            Some(cs.get(Colors::Selected))
        } else {
            None
        }
    }
}

fn selection_state_for(some_id: ID) -> SelectionState {
    match some_id {
        ID::Intersection(id) => SelectionState::SelectedIntersection(id),
        ID::Lane(id) => SelectionState::SelectedLane(id),
        ID::Building(id) => SelectionState::SelectedBuilding(id),
        ID::Turn(id) => SelectionState::SelectedTurn(id),
        ID::Car(id) => SelectionState::SelectedCar(id),
        ID::Pedestrian(id) => SelectionState::SelectedPedestrian(id),
        ID::ExtraShape(id) => SelectionState::SelectedExtraShape(id),
    }
}

pub struct Hider {
    items: HashSet<ID>,
}

impl Hider {
    pub fn new() -> Hider {
        Hider {
            items: HashSet::new(),
        }
    }

    pub fn event(&mut self, input: &mut UserInput, state: &mut SelectionState) -> bool {
        if input.unimportant_key_pressed(Key::K, "unhide everything") {
            println!("Unhiding {} things", self.items.len());
            self.items.clear();
            return true;
        }

        let item = match state {
            SelectionState::SelectedIntersection(id) => Some(ID::Intersection(*id)),
            SelectionState::SelectedLane(id) => Some(ID::Lane(*id)),
            SelectionState::SelectedBuilding(id) => Some(ID::Building(*id)),
            SelectionState::SelectedExtraShape(id) => Some(ID::ExtraShape(*id)),
            _ => None,
        };
        if let Some(id) = item {
            if input.unimportant_key_pressed(Key::H, &format!("hide {:?}", id)) {
                self.items.insert(id);
                println!("Hiding {:?}", id);
                *state = SelectionState::Empty;
                return true;
            }
        }
        false
    }

    pub fn show_l(&self, id: LaneID) -> bool {
        !self.items.contains(&ID::Lane(id))
    }

    pub fn show_b(&self, id: BuildingID) -> bool {
        !self.items.contains(&ID::Building(id))
    }

    pub fn show_i(&self, id: IntersectionID) -> bool {
        !self.items.contains(&ID::Intersection(id))
    }

    pub fn show_es(&self, id: ExtraShapeID) -> bool {
        !self.items.contains(&ID::ExtraShape(id))
    }
}

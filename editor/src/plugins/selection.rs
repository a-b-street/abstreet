// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use control::ControlMap;
use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use ezgui::GfxCtx;
use graphics::types::Color;
use map_model;
use map_model::{BuildingID, IntersectionID, LaneID, Map, TurnID};
use piston::input::{Button, Key, ReleaseEvent};
use render;
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
    //Parcel(ParcelID),
}

// TODO lots of code duplication here, but haven't quite worked out how to improve it...

#[derive(Clone)]
pub enum SelectionState {
    Empty,
    SelectedIntersection(IntersectionID),
    // Second param is the current_turn_index
    SelectedLane(LaneID, Option<usize>),
    SelectedBuilding(BuildingID),
    SelectedTurn(TurnID),
    SelectedCar(CarID),
    SelectedPedestrian(PedestrianID),
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

    pub fn event(&mut self, input: &mut UserInput, map: &Map, sim: &Sim) -> bool {
        let mut new_state: Option<SelectionState> = None;
        let active = match self {
            SelectionState::SelectedLane(id, current_turn_index) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Lane(*id)));
                    true
                } else if input.key_pressed(Key::Tab, "cycle through this lane's turns") {
                    let idx = match *current_turn_index {
                        Some(i) => i + 1,
                        None => 0,
                    };
                    new_state = Some(SelectionState::SelectedLane(*id, Some(idx)));
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
                    true
                } else {
                    false
                }
            }
            SelectionState::SelectedBuilding(id) => {
                if input.key_pressed(Key::LCtrl, &format!("Hold Ctrl to show {}'s tooltip", id)) {
                    new_state = Some(SelectionState::Tooltip(ID::Building(*id)));
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
        control_map: &ControlMap,
        sim: &Sim,
        cs: &ColorScheme,
        g: &mut GfxCtx,
    ) {
        match *self {
            SelectionState::Empty
            | SelectionState::SelectedTurn(_)
            | SelectionState::SelectedBuilding(_)
            | SelectionState::SelectedCar(_)
            | SelectionState::SelectedPedestrian(_) => {}
            SelectionState::SelectedIntersection(id) => {
                if let Some(signal) = control_map.traffic_signals.get(&id) {
                    let (cycle, _) = signal.current_cycle_and_remaining_time(sim.time.as_time());
                    for t in &cycle.turns {
                        draw_map.get_t(*t).draw_full(g, cs.get(Colors::Turn));
                    }
                }
            }
            SelectionState::SelectedLane(id, current_turn_index) => {
                let relevant_turns = map.get_turns_from_lane(id);
                if !relevant_turns.is_empty() {
                    match current_turn_index {
                        Some(idx) => {
                            let turn = map.get_t(relevant_turns[idx % relevant_turns.len()].id);
                            let draw_turn =
                                draw_map.get_t(relevant_turns[idx % relevant_turns.len()].id);
                            draw_turn.draw_full(g, cs.get(Colors::Turn));

                            for t in map.get_turns_in_intersection(turn.parent) {
                                if t.conflicts_with(turn) {
                                    let draw_t = draw_map.get_t(t.id);
                                    // TODO should we instead change color_t?
                                    draw_t.draw_icon(g, cs.get(Colors::ConflictingTurn), cs);
                                }
                            }
                        }
                        None => for turn in &relevant_turns {
                            draw_map.get_t(turn.id).draw_full(g, cs.get(Colors::Turn));
                        },
                    }
                }
                //draw_map.get_l(id).draw_debug(g, cs, map.get_l(id));
            }
            SelectionState::Tooltip(some_id) => {
                let lines = match some_id {
                    ID::Lane(id) => draw_map.get_l(id).tooltip_lines(map),
                    ID::Building(id) => draw_map.get_b(id).tooltip_lines(map),
                    ID::Car(id) => sim.car_tooltip(id),
                    ID::Pedestrian(id) => sim.ped_tooltip(id),
                    ID::Intersection(id) => vec![format!("{}", id)],
                    ID::Turn(id) => vec![format!("{}", id)],
                };
                canvas.draw_mouse_tooltip(g, &lines);
            }
        }
    }

    // TODO instead, since color logic is complicated anyway, just have a way to ask "are we
    // selecting this generic ID?"

    pub fn color_l(&self, l: &map_model::Lane, cs: &ColorScheme) -> Option<Color> {
        match *self {
            SelectionState::SelectedLane(id, _) if l.id == id => Some(cs.get(Colors::Selected)),
            SelectionState::Tooltip(ID::Lane(id)) if l.id == id => Some(cs.get(Colors::Selected)),
            _ => None,
        }
    }
    pub fn color_i(&self, i: &map_model::Intersection, cs: &ColorScheme) -> Option<Color> {
        match *self {
            SelectionState::SelectedIntersection(id) if i.id == id => {
                Some(cs.get(Colors::Selected))
            }
            SelectionState::Tooltip(ID::Intersection(id)) if i.id == id => {
                Some(cs.get(Colors::Selected))
            }
            _ => None,
        }
    }
    pub fn color_t(&self, t: &map_model::Turn, cs: &ColorScheme) -> Option<Color> {
        match *self {
            SelectionState::SelectedTurn(id) if t.id == id => Some(cs.get(Colors::Selected)),
            SelectionState::Tooltip(ID::Turn(id)) if t.id == id => Some(cs.get(Colors::Selected)),
            _ => None,
        }
    }
    pub fn color_b(&self, b: &map_model::Building, cs: &ColorScheme) -> Option<Color> {
        match *self {
            SelectionState::SelectedBuilding(id) if b.id == id => Some(cs.get(Colors::Selected)),
            SelectionState::Tooltip(ID::Building(id)) if b.id == id => {
                Some(cs.get(Colors::Selected))
            }
            _ => None,
        }
    }
    pub fn color_c(&self, c: CarID, cs: &ColorScheme) -> Option<Color> {
        match *self {
            SelectionState::SelectedCar(id) if c == id => Some(cs.get(Colors::Selected)),
            SelectionState::Tooltip(ID::Car(id)) if c == id => Some(cs.get(Colors::Selected)),
            _ => None,
        }
    }

    pub fn color_p(&self, p: PedestrianID, cs: &ColorScheme) -> Option<Color> {
        match *self {
            SelectionState::SelectedPedestrian(id) if p == id => Some(cs.get(Colors::Selected)),
            SelectionState::Tooltip(ID::Pedestrian(id)) if p == id => {
                Some(cs.get(Colors::Selected))
            }
            _ => None,
        }
    }
}

fn selection_state_for(some_id: ID) -> SelectionState {
    match some_id {
        ID::Intersection(id) => SelectionState::SelectedIntersection(id),
        ID::Lane(id) => SelectionState::SelectedLane(id, None),
        ID::Building(id) => SelectionState::SelectedBuilding(id),
        ID::Turn(id) => SelectionState::SelectedTurn(id),
        ID::Car(id) => SelectionState::SelectedCar(id),
        ID::Pedestrian(id) => SelectionState::SelectedPedestrian(id),
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
            SelectionState::SelectedLane(id, _) => Some(ID::Lane(*id)),
            SelectionState::SelectedBuilding(id) => Some(ID::Building(*id)),
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
}

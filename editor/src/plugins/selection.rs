// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use animation;
use control::ControlMap;
use ezgui::canvas::{Canvas, GfxCtx};
use ezgui::input::UserInput;
use geom;
use graphics::types::Color;
use map_model;
use map_model::{BuildingID, IntersectionID, Map, RoadID, TurnID};
use piston::input::{Button, Key, ReleaseEvent};
use render;
use sim::CarID;
use sim::straw_model::Sim;

// TODO only used for mouseover, which happens in order anyway...
pub enum ID {
    Road(RoadID),
    Intersection(IntersectionID),
    Turn(TurnID),
    Building(BuildingID),
    Car(CarID),
    //Parcel(ParcelID),
}

pub enum SelectionState {
    Empty,
    SelectedIntersection(IntersectionID),
    // Second param is the current_turn_index
    SelectedRoad(RoadID, Option<usize>),
    TooltipRoad(RoadID),
    SelectedBuilding(BuildingID),
    SelectedTurn(TurnID),
    SelectedCar(CarID),
}

impl SelectionState {
    // TODO shouldnt these two consume self?
    pub fn handle_mouseover(&self, some_id: &Option<ID>) -> SelectionState {
        match *some_id {
            Some(ID::Intersection(id)) => SelectionState::SelectedIntersection(id),
            Some(ID::Road(id)) => {
                match *self {
                    // Don't break out of the tooltip state
                    SelectionState::TooltipRoad(_) => SelectionState::TooltipRoad(id),
                    _ => SelectionState::SelectedRoad(id, None),
                }
            }
            Some(ID::Building(id)) => SelectionState::SelectedBuilding(id),
            Some(ID::Turn(id)) => SelectionState::SelectedTurn(id),
            Some(ID::Car(id)) => SelectionState::SelectedCar(id),
            None => SelectionState::Empty,
        }
    }

    // TODO consume self
    pub fn event(
        &self,
        input: &mut UserInput,
        sim: &mut Sim,
    ) -> (SelectionState, animation::EventLoopMode) {
        // TODO simplify the way this is written
        match *self {
            SelectionState::Empty => (SelectionState::Empty, animation::EventLoopMode::InputOnly),
            SelectionState::SelectedIntersection(id) => (
                SelectionState::SelectedIntersection(id),
                animation::EventLoopMode::InputOnly,
            ),
            SelectionState::SelectedRoad(id, current_turn_index) => {
                if input.key_pressed(
                    Key::LCtrl,
                    &format!("Hold Ctrl to show road {:?}'s tooltip", id),
                ) {
                    (
                        SelectionState::TooltipRoad(id),
                        animation::EventLoopMode::InputOnly,
                    )
                } else if input
                    .key_pressed(Key::Tab, "Press Tab to cycle through this road's turns")
                {
                    let idx = match current_turn_index {
                        Some(i) => i + 1,
                        None => 0,
                    };
                    (
                        SelectionState::SelectedRoad(id, Some(idx)),
                        animation::EventLoopMode::InputOnly,
                    )
                } else {
                    (
                        SelectionState::SelectedRoad(id, current_turn_index),
                        animation::EventLoopMode::InputOnly,
                    )
                }
            }
            SelectionState::TooltipRoad(id) => {
                if let Some(Button::Keyboard(Key::LCtrl)) =
                    input.use_event_directly().release_args()
                {
                    (
                        SelectionState::SelectedRoad(id, None),
                        animation::EventLoopMode::InputOnly,
                    )
                } else {
                    (
                        SelectionState::TooltipRoad(id),
                        animation::EventLoopMode::InputOnly,
                    )
                }
            }
            SelectionState::SelectedBuilding(id) => (
                SelectionState::SelectedBuilding(id),
                animation::EventLoopMode::InputOnly,
            ),
            SelectionState::SelectedTurn(id) => (
                SelectionState::SelectedTurn(id),
                animation::EventLoopMode::InputOnly,
            ),
            SelectionState::SelectedCar(id) => {
                // TODO not sure if we should debug like this (pushing the bit down to all the
                // layers representing an entity) or by using some scary global mutable singleton
                if input.unimportant_key_pressed(Key::D, "press D to debug") {
                    sim.toggle_debug(id);
                }

                (
                    SelectionState::SelectedCar(id),
                    animation::EventLoopMode::InputOnly,
                )
            }
        }
    }

    pub fn draw(
        &self,
        map: &Map,
        canvas: &Canvas,
        geom_map: &geom::GeomMap,
        draw_map: &render::DrawMap,
        control_map: &ControlMap,
        sim: &Sim,
        g: &mut GfxCtx,
    ) {
        match *self {
            SelectionState::Empty | SelectionState::SelectedTurn(_) => {}
            SelectionState::SelectedIntersection(id) => {
                if let Some(signal) = control_map.traffic_signals.get(&id) {
                    let (cycle, _) = signal.current_cycle_and_remaining_time(sim.time.as_time());
                    for t in &cycle.turns {
                        draw_map.get_t(*t).draw_full(g, render::TURN_COLOR);
                    }
                }
            }
            SelectionState::SelectedRoad(id, current_turn_index) => {
                let all_turns: Vec<&map_model::Turn> =
                    map.get_turns_in_intersection(map.get_destination_intersection(id).id);
                let relevant_turns = map.get_turns_from_road(id);
                match current_turn_index {
                    Some(idx) => {
                        let turn = draw_map.get_t(relevant_turns[idx % relevant_turns.len()].id);
                        let geom_turn =
                            geom_map.get_t(relevant_turns[idx % relevant_turns.len()].id);
                        turn.draw_full(g, render::TURN_COLOR);
                        for map_t in all_turns {
                            let draw_t = draw_map.get_t(map_t.id);
                            let geom_t = geom_map.get_t(map_t.id);
                            if geom_t.conflicts_with(geom_turn) {
                                // TODO should we instead change color_t?
                                draw_t.draw_icon(g, render::CONFLICTING_TURN_COLOR);
                            }
                        }
                    }
                    None => for turn in &relevant_turns {
                        draw_map.get_t(turn.id).draw_full(g, render::TURN_COLOR);
                    },
                }
            }
            SelectionState::TooltipRoad(id) => {
                canvas.draw_mouse_tooltip(g, &draw_map.get_r(id).tooltip_lines(map, geom_map));
            }
            SelectionState::SelectedBuilding(id) => {
                canvas.draw_mouse_tooltip(g, &draw_map.get_b(id).tooltip_lines(map));
            }
            SelectionState::SelectedCar(id) => {
                canvas.draw_mouse_tooltip(g, &sim.car_tooltip(id));
            }
        }
    }

    // TODO instead, since color logic is complicated anyway, just have a way to ask "are we
    // selecting this generic ID?"

    pub fn color_r(&self, r: &map_model::Road) -> Option<Color> {
        match *self {
            SelectionState::SelectedRoad(id, _) if r.id == id => Some(render::SELECTED_COLOR),
            SelectionState::TooltipRoad(id) if r.id == id => Some(render::SELECTED_COLOR),
            _ => None,
        }
    }
    pub fn color_i(&self, i: &map_model::Intersection) -> Option<Color> {
        match *self {
            SelectionState::SelectedIntersection(id) if i.id == id => Some(render::SELECTED_COLOR),
            _ => None,
        }
    }
    pub fn color_t(&self, t: &map_model::Turn) -> Option<Color> {
        match *self {
            SelectionState::SelectedTurn(id) if t.id == id => Some(render::SELECTED_COLOR),
            _ => None,
        }
    }
    pub fn color_b(&self, b: &map_model::Building) -> Option<Color> {
        match *self {
            SelectionState::SelectedBuilding(id) if b.id == id => Some(render::SELECTED_COLOR),
            _ => None,
        }
    }
    pub fn color_c(&self, c: CarID) -> Option<Color> {
        match *self {
            SelectionState::SelectedCar(id) if c == id => Some(render::SELECTED_COLOR),
            _ => None,
        }
    }
}

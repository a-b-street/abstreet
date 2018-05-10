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

extern crate map_model;

use control::ControlMap;
use control::stop_signs::TurnPriority;
use ezgui::canvas;
use ezgui::input::UserInput;
use geom::GeomMap;
use graphics::types::Color;
use map_model::IntersectionID;
use map_model::{Map, Turn};
use piston::input::Key;
use plugins::selection::SelectionState;

pub struct StopSignEditor {
    i: IntersectionID,
}

impl StopSignEditor {
    pub fn new(i: IntersectionID) -> StopSignEditor {
        StopSignEditor { i }
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        map: &Map,
        geom_map: &GeomMap,
        control_map: &mut ControlMap,
        current_selection: &SelectionState,
    ) -> bool {
        if input.key_pressed(Key::Return, "Press enter to quit the editor") {
            return true;
        }

        if let SelectionState::SelectedTurn(id) = *current_selection {
            if map.get_t(id).parent == self.i {
                let sign = &mut control_map.stop_signs.get_mut(&self.i).unwrap();
                match sign.get_priority(id) {
                    TurnPriority::Priority => {
                        if input.key_pressed(Key::D2, "Press 2 to make this turn yield") {
                            sign.set_priority(id, TurnPriority::Yield, geom_map);
                        }
                        if input.key_pressed(Key::D3, "Press 3 to make this turn always stop") {
                            sign.set_priority(id, TurnPriority::Stop, geom_map);
                        }
                    }
                    TurnPriority::Yield => {
                        if sign.could_be_priority_turn(id, geom_map)
                            && input.key_pressed(Key::D1, "Press 1 to let this turn go immediately")
                        {
                            sign.set_priority(id, TurnPriority::Priority, geom_map);
                        }
                        if input.key_pressed(Key::D3, "Press 3 to make this turn always stop") {
                            sign.set_priority(id, TurnPriority::Stop, geom_map);
                        }
                    }
                    TurnPriority::Stop => {
                        if sign.could_be_priority_turn(id, geom_map)
                            && input.key_pressed(Key::D1, "Press 1 to let this turn go immediately")
                        {
                            sign.set_priority(id, TurnPriority::Priority, geom_map);
                        }
                        if input.key_pressed(Key::D2, "Press 2 to make this turn yield") {
                            sign.set_priority(id, TurnPriority::Yield, geom_map);
                        }
                    }
                };
            }
        }

        false
    }

    pub fn color_t(&self, t: &Turn, control_map: &ControlMap) -> Option<Color> {
        if t.parent != self.i {
            return Some(canvas::DARK_GREY);
        }
        match control_map.stop_signs[&self.i].get_priority(t.id) {
            TurnPriority::Priority => Some(canvas::GREEN),
            TurnPriority::Yield => Some(canvas::YELLOW),
            TurnPriority::Stop => Some(canvas::RED),
        }
    }
}

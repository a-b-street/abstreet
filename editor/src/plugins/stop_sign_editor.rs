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

use map_model::Map;
use plugins::selection::SelectionState;
use control::ControlMap;
use map_model::IntersectionID;
use render::DrawMap;
use ezgui::input::UserInput;
use piston::input::Key;

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
        draw_map: &DrawMap,
        control_map: &mut ControlMap,
        current_selection: &SelectionState,
    ) -> bool {
        if input.key_pressed(Key::Return, "Press enter to quit the editor") {
            return true;
        }

        if let SelectionState::SelectedRoadIcon(id) = *current_selection {
            if map.get_destination_intersection(id).id == self.i {
                let sign = &mut control_map.stop_signs.get_mut(&self.i).unwrap();
                if sign.is_priority_road(id) {
                    if input.key_pressed(
                        Key::Backspace,
                        "Press Backspace to make this road always stop",
                    ) {
                        sign.remove_priority_road(id);
                    }
                } else if sign.could_be_priority_road(id, &draw_map.turns) {
                    if input.key_pressed(
                        Key::Space,
                        "Press Space to let this road proceed without stopping",
                    ) {
                        sign.add_priority_road(id);
                    }
                }
            }
        }

        false
    }
}

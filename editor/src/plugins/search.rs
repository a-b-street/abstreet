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

use ezgui::canvas::{Canvas, GfxCtx};
use ezgui::input::UserInput;
use ezgui::text_box::TextBox;
use graphics::types::Color;
use map_model;
use piston::input::Key;
use render;

pub enum SearchState {
    Empty,
    EnteringSearch(TextBox),
    FilterOSM(String),
}

impl SearchState {
    pub fn color_r(&self, r: &map_model::Road) -> Option<Color> {
        self.choose_color(&r.osm_tags)
    }
    pub fn color_b(&self, b: &map_model::Building) -> Option<Color> {
        self.choose_color(&b.osm_tags)
    }

    fn choose_color(&self, osm_tags: &[String]) -> Option<Color> {
        if let SearchState::FilterOSM(filter) = self {
            for tag in osm_tags {
                if tag.contains(filter) {
                    return Some(render::SEARCH_RESULT_COLOR);
                }
            }
        }
        None
    }

    // TODO Does this pattern where we consume self and return it work out nicer?
    pub fn event(self, input: &mut UserInput) -> SearchState {
        match self {
            SearchState::Empty => {
                if input.unimportant_key_pressed(Key::Slash, "Press / to start searching") {
                    SearchState::EnteringSearch(TextBox::new())
                } else {
                    self
                }
            }
            SearchState::EnteringSearch(mut tb) => {
                if tb.event(input.use_event_directly()) {
                    input.consume_event();
                    SearchState::FilterOSM(tb.line)
                } else {
                    input.consume_event();
                    SearchState::EnteringSearch(tb)
                }
            }
            SearchState::FilterOSM(filter) => {
                if input.key_pressed(
                    Key::Return,
                    &format!("Press enter to clear the current search for {}", filter),
                ) {
                    SearchState::Empty
                } else {
                    SearchState::FilterOSM(filter)
                }
            }
        }
    }

    pub fn draw(&self, canvas: &Canvas, g: &mut GfxCtx) {
        if let SearchState::EnteringSearch(text_box) = self {
            canvas.draw_osd_notification(g, &vec![text_box.line.clone()]);
            // TODO draw the cursor
        }
    }
}

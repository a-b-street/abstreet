// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::input::UserInput;
use ezgui::text_box::TextBox;
use graphics::types::Color;
use map_model;
use piston::input::Key;
use std::collections::HashMap;

pub enum SearchState {
    Empty,
    EnteringSearch(TextBox),
    FilterOSM(String),
}

impl SearchState {
    pub fn color_r(&self, r: &map_model::Road, cs: &ColorScheme) -> Option<Color> {
        self.choose_color(&r.osm_tags, cs)
    }
    pub fn color_b(&self, b: &map_model::Building, cs: &ColorScheme) -> Option<Color> {
        self.choose_color(&b.osm_tags, cs)
    }

    fn choose_color(&self, osm_tags: &HashMap<String, String>, cs: &ColorScheme) -> Option<Color> {
        if let SearchState::FilterOSM(filter) = self {
            for (k, v) in osm_tags {
                if format!("{}={}", k, v).contains(filter) {
                    return Some(cs.get(Colors::SearchResult));
                }
            }
        }
        None
    }

    pub fn event(&mut self, input: &mut UserInput) -> bool {
        match self {
            SearchState::Empty => {
                if input.unimportant_key_pressed(Key::Slash, "Press / to start searching") {
                    *self = SearchState::EnteringSearch(TextBox::new());
                    return true;
                }
                false
            }
            SearchState::EnteringSearch(tb) => {
                if tb.event(input.use_event_directly()) {
                    *self = SearchState::FilterOSM(tb.line.clone());
                }
                input.consume_event();
                true
            }
            SearchState::FilterOSM(filter) => {
                if input.key_pressed(
                    Key::Return,
                    &format!("Press enter to clear the current search for {}", filter),
                ) {
                    *self = SearchState::Empty;
                }
                true
            }
        }
    }

    pub fn get_osd_lines(&self) -> Vec<String> {
        // TODO draw the cursor
        if let SearchState::EnteringSearch(text_box) = self {
            return vec![text_box.line.clone()];
        }
        Vec::new()
    }
}

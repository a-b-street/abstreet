// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::{TextBox, UserInput};
use graphics::types::Color;
use objects::ID;
use piston::input::Key;
use plugins::{Colorizer, Ctx};
use std::collections::BTreeMap;

pub enum SearchState {
    Empty,
    EnteringSearch(TextBox),
    FilterOSM(String),
}

impl SearchState {
    fn choose_color(&self, osm_tags: &BTreeMap<String, String>, cs: &ColorScheme) -> Option<Color> {
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
        let mut new_state: Option<SearchState> = None;
        match self {
            SearchState::Empty => {
                if input.unimportant_key_pressed(Key::Slash, "start searching") {
                    new_state = Some(SearchState::EnteringSearch(TextBox::new()));
                }
            }
            SearchState::EnteringSearch(tb) => {
                if tb.event(input.use_event_directly()) {
                    new_state = Some(SearchState::FilterOSM(tb.line.clone()));
                }
                input.consume_event();
            }
            SearchState::FilterOSM(filter) => {
                if input.key_pressed(
                    Key::Return,
                    &format!("clear the current search for {}", filter),
                ) {
                    new_state = Some(SearchState::Empty);
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            SearchState::Empty => false,
            _ => true,
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

impl Colorizer for SearchState {
    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match obj {
            ID::Lane(l) => self.choose_color(&ctx.map.get_parent(l).osm_tags, ctx.cs),
            ID::Building(b) => self.choose_color(&ctx.map.get_b(b).osm_tags, ctx.cs),
            _ => None,
        }
    }
}

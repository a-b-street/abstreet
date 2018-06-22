// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::input::UserInput;
use graphics::types::Color;
use map_model;
use piston::input::Key;

// TODO have some UI for editing these rules and saving them
pub struct OsmClassifier {
    active: bool,
}

impl OsmClassifier {
    pub fn new() -> OsmClassifier {
        OsmClassifier { active: false }
    }

    pub fn handle_event(&mut self, input: &mut UserInput) {
        let msg = if self.active {
            "Press 6 to stop showing OSM classes"
        } else {
            "Press 6 to show OSM classifications"
        };
        if input.unimportant_key_pressed(Key::D6, msg) {
            self.active = !self.active;
        }
    }

    pub fn color_r(&self, r: &map_model::Road, cs: &ColorScheme) -> Option<Color> {
        if !self.active {
            return None;
        }

        for tag in &r.osm_tags {
            if tag == "highway=primary" || tag == "highway=secondary" || tag == "highway=tertiary" {
                return Some(cs.get(Colors::MatchClassification));
            }
        }
        Some(cs.get(Colors::DontMatchClassification))
    }
    pub fn color_b(&self, b: &map_model::Building, cs: &ColorScheme) -> Option<Color> {
        if !self.active {
            return None;
        }

        for tag in &b.osm_tags {
            if tag.contains("addr:housenumber") {
                return Some(cs.get(Colors::MatchClassification));
            }
        }
        None
    }
}

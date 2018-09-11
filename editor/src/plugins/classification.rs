// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::UserInput;
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

    pub fn handle_event(&mut self, input: &mut UserInput) -> bool {
        let msg = if self.active {
            "stop showing OSM classes"
        } else {
            "to show OSM classifications"
        };
        if input.unimportant_key_pressed(Key::D6, msg) {
            self.active = !self.active;
            true
        } else {
            false
        }
    }

    pub fn color_l(
        &self,
        l: &map_model::Lane,
        map: &map_model::Map,
        cs: &ColorScheme,
    ) -> Option<Color> {
        if !self.active {
            return None;
        }

        if match map.get_r(l.parent).osm_tags.get("highway") {
            Some(hwy) => hwy == "primary" || hwy == "secondary" || hwy == "tertiary",
            None => false,
        } {
            Some(cs.get(Colors::MatchClassification))
        } else {
            Some(cs.get(Colors::DontMatchClassification))
        }
    }
    pub fn color_b(&self, b: &map_model::Building, cs: &ColorScheme) -> Option<Color> {
        if !self.active {
            return None;
        }

        if b.osm_tags.contains_key("addr:housenumber") {
            Some(cs.get(Colors::MatchClassification))
        } else {
            None
        }
    }
}

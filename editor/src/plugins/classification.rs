// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::Colors;
use ezgui::UserInput;
use graphics::types::Color;
use objects::{Ctx, DEBUG_EXTRA, ID};
use piston::input::Key;
use plugins::Colorizer;

// TODO have some UI for editing these rules and saving them
pub struct OsmClassifier {
    active: bool,
}

impl OsmClassifier {
    pub fn new() -> OsmClassifier {
        OsmClassifier { active: false }
    }

    pub fn event(&mut self, input: &mut UserInput) -> bool {
        let msg = if self.active {
            "stop showing OSM classes"
        } else {
            "to show OSM classifications"
        };
        if input.unimportant_key_pressed(Key::D6, DEBUG_EXTRA, msg) {
            self.active = !self.active;
        }
        self.active
    }
}

impl Colorizer for OsmClassifier {
    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        if !self.active {
            return None;
        }

        match obj {
            ID::Lane(l) => {
                if match ctx.map.get_parent(l).osm_tags.get("highway") {
                    Some(hwy) => hwy == "primary" || hwy == "secondary" || hwy == "tertiary",
                    None => false,
                } {
                    Some(ctx.cs.get(Colors::MatchClassification))
                } else {
                    Some(ctx.cs.get(Colors::DontMatchClassification))
                }
            }
            ID::Building(b) => if ctx.map.get_b(b).osm_tags.contains_key("addr:housenumber") {
                Some(ctx.cs.get(Colors::MatchClassification))
            } else {
                None
            },
            _ => None,
        }
    }
}

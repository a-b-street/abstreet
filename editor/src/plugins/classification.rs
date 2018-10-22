// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::Colors;
use ezgui::Color;
use objects::{Ctx, DEBUG_EXTRA, ID};
use piston::input::Key;
use plugins::{Plugin, PluginCtx};

// TODO have some UI for editing these rules and saving them
pub struct OsmClassifier {
    active: bool,
}

impl OsmClassifier {
    pub fn new() -> OsmClassifier {
        OsmClassifier { active: false }
    }
}

impl Plugin for OsmClassifier {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let msg = if self.active {
            "stop showing OSM classes"
        } else {
            "to show OSM classifications"
        };
        if ctx.input.unimportant_key_pressed(Key::D6, DEBUG_EXTRA, msg) {
            self.active = !self.active;
        }
        self.active
    }

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

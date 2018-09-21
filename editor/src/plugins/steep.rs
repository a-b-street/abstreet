// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO check out https://accessmap.io/ for inspiration on how to depict elevation

use ezgui::UserInput;
use graphics::types::Color;
use map_model::{Lane, Map};
use objects::{Ctx, DEBUG_EXTRA, ID};
use piston::input::Key;
use plugins::Colorizer;
use std::f64;

pub struct SteepnessVisualizer {
    active: bool,
    min_difference: f64,
    max_difference: f64,
}

impl SteepnessVisualizer {
    pub fn new(map: &Map) -> SteepnessVisualizer {
        let mut s = SteepnessVisualizer {
            active: false,
            min_difference: f64::MAX,
            max_difference: f64::MIN_POSITIVE,
        };
        for l in map.all_lanes() {
            let d = s.get_delta(map, l);
            // TODO hack! skip crazy outliers in terrible way.
            if d > 100.0 {
                continue;
            }
            s.min_difference = s.min_difference.min(d);
            s.max_difference = s.max_difference.max(d);
        }
        s
    }

    pub fn event(&mut self, input: &mut UserInput) -> bool {
        let msg = if self.active {
            "stop showing steepness"
        } else {
            "visualize steepness"
        };
        if input.unimportant_key_pressed(Key::D5, DEBUG_EXTRA, msg) {
            self.active = !self.active;
        }
        self.active
    }

    fn get_delta(&self, map: &Map, l: &Lane) -> f64 {
        let e1 = map.get_source_intersection(l.id).elevation;
        let e2 = map.get_destination_intersection(l.id).elevation;
        (e1 - e2).value_unsafe.abs()
    }
}

impl Colorizer for SteepnessVisualizer {
    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        if !self.active {
            return None;
        }

        match obj {
            ID::Lane(l) => {
                let normalized = (self.get_delta(ctx.map, ctx.map.get_l(l)) - self.min_difference)
                    / (self.max_difference - self.min_difference);
                Some([normalized as f32, 0.0, 0.0, 1.0])
            }
            _ => None,
        }
    }
}

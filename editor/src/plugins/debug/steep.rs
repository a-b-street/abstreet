// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO check out https://accessmap.io/ for inspiration on how to depict elevation

use crate::objects::{Ctx, DEBUG_EXTRA, ID};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::Color;
use map_model::Map;
use piston::input::Key;
use std::f64;

pub struct SteepnessVisualizer {
    pub active: bool,
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
        for r in map.all_roads() {
            let d = (map.get_i(r.src_i).elevation - map.get_i(r.dst_i).elevation)
                .value_unsafe
                .abs();
            // TODO hack! skip crazy outliers in terrible way.
            if d > 100.0 {
                continue;
            }
            s.min_difference = s.min_difference.min(d);
            s.max_difference = s.max_difference.max(d);
        }
        s
    }
}

impl Plugin for SteepnessVisualizer {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let msg = if self.active {
            "stop showing steepness"
        } else {
            "visualize steepness"
        };
        if ctx.input.unimportant_key_pressed(Key::D5, DEBUG_EXTRA, msg) {
            self.active = !self.active;
        }
        self.active
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        if !self.active {
            return None;
        }

        match obj {
            ID::Lane(l) => {
                let e1 = ctx.map.get_source_intersection(l).elevation;
                let e2 = ctx.map.get_destination_intersection(l).elevation;
                let d = (e1 - e2).value_unsafe.abs();
                let normalized =
                    (d - self.min_difference) / (self.max_difference - self.min_difference);
                Some(Color::rgb_f(normalized as f32, 0.0, 0.0))
            }
            _ => None,
        }
    }
}

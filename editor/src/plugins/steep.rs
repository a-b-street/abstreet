// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO check out https://accessmap.io/ for inspiration on how to depict elevation

use ezgui::input::UserInput;
use graphics::types::Color;
use map_model::{Map, Road};
use piston::input::Key;
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
        for r in map.all_roads() {
            let d = s.get_delta(map, r);
            // TODO hack! skip crazy outliers in terrible way.
            if d > 100.0 {
                continue;
            }
            s.min_difference = s.min_difference.min(d);
            s.max_difference = s.max_difference.max(d);
        }
        s
    }

    pub fn handle_event(&mut self, input: &mut UserInput) -> bool {
        let msg = if self.active {
            "Press 5 to stop showing steepness"
        } else {
            "Press 5 to visualize steepness"
        };
        if input.unimportant_key_pressed(Key::D5, msg) {
            self.active = !self.active;
            true
        } else {
            false
        }
    }

    fn get_delta(&self, map: &Map, r: &Road) -> f64 {
        let e1 = map.get_source_intersection(r.id).elevation;
        let e2 = map.get_destination_intersection(r.id).elevation;
        (e1 - e2).value_unsafe.abs()
    }

    pub fn color_r(&self, map: &Map, r: &Road) -> Option<Color> {
        if !self.active {
            return None;
        }

        let normalized = (self.get_delta(map, r) - self.min_difference)
            / (self.max_difference - self.min_difference);
        Some([normalized as f32, 0.0, 0.0, 1.0])
    }
}

// TODO uh oh, we need Map again
/*impl ColorChooser for SteepnessVisualizer {
    fn color_r(&self, r: &Road) -> Option<Color> {
        let normalized = (self.get_delta(&r) - self.min_difference) /
          (self.max_difference - self.min_difference);
        return Some([normalized as f32, 0.0, 0.0, 1.0]);
    }
}*/

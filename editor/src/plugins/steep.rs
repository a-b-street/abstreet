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

// TODO check out https://accessmap.io/ for inspiration on how to depict elevation

extern crate map_model;

use graphics::types::Color;
use map_model::{Map, Road};
use std::f64;

pub struct SteepnessVisualizer {
    min_difference: f64,
    max_difference: f64,
}

impl SteepnessVisualizer {
    pub fn new(map: &Map) -> SteepnessVisualizer {
        let mut s = SteepnessVisualizer {
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

    fn get_delta(&self, map: &Map, r: &Road) -> f64 {
        let e1 = map.get_source_intersection(r.id).elevation_meters;
        let e2 = map.get_destination_intersection(r.id).elevation_meters;
        (e1 - e2).abs()
    }

    pub fn color_r(&self, map: &Map, r: &Road) -> Option<Color> {
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

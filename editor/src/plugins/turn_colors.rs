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

// TODO problem with this approach is that turns can belong to multiple cycles

extern crate map_model;

use graphics::types::Color;
use map_model::{Turn, TurnID};
use render::ColorChooser;
use control::ControlMap;
use std::collections::HashMap;

const CYCLE_COLORS: [Color; 8] = [
    // TODO these are awful choices
    [1.0, 1.0, 0.0, 1.0],
    [1.0, 0.0, 1.0, 1.0],
    [0.0, 1.0, 1.0, 1.0],
    [0.5, 0.2, 0.7, 1.0],
    [0.5, 0.5, 0.0, 0.5],
    [0.5, 0.0, 0.5, 0.5],
    [0.0, 0.5, 0.5, 0.5],
    [0.0, 0.0, 0.5, 0.5],
];

pub struct TurnColors {
    cycle_idx_per_turn: HashMap<TurnID, usize>,
}

impl TurnColors {
    pub fn new(map: &ControlMap) -> TurnColors {
        let mut m = HashMap::new();
        for signal in map.traffic_signals.values() {
            for (idx, cycle) in signal.cycles.iter().enumerate() {
                for t in &cycle.turns {
                    m.insert(*t, idx);
                }
            }
        }
        TurnColors {
            cycle_idx_per_turn: m,
        }
    }
}

impl ColorChooser for TurnColors {
    fn color_t(&self, t: &Turn) -> Option<Color> {
        if let Some(cycle) = self.cycle_idx_per_turn.get(&t.id) {
            return Some(CYCLE_COLORS[*cycle]);
        }
        None
    }
}

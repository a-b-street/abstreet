// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO problem with this approach is that turns can belong to multiple cycles

use control::ControlMap;
use graphics::types::Color;
use map_model::{Turn, TurnID};
use std::collections::HashMap;

const CYCLE_COLORS: [Color; 14] = [
    // TODO these are awful choices
    [1.0, 1.0, 0.0, 1.0],
    [1.0, 0.0, 1.0, 1.0],
    [0.0, 1.0, 1.0, 1.0],
    [0.5, 0.2, 0.7, 1.0],
    [0.5, 0.5, 0.0, 0.5],
    [0.5, 0.0, 0.5, 0.5],
    [0.0, 0.5, 0.5, 0.5],
    [0.0, 0.0, 0.5, 0.5],
    [0.3, 0.2, 0.5, 0.5],
    [0.4, 0.2, 0.5, 0.5],
    [0.5, 0.2, 0.5, 0.5],
    [0.6, 0.2, 0.5, 0.5],
    [0.7, 0.2, 0.5, 0.5],
    [0.8, 0.2, 0.5, 0.5],
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
                    if idx >= CYCLE_COLORS.len() {
                        panic!(
                            "Turn idx {}, but only {} CYCLE_COLORS",
                            idx,
                            CYCLE_COLORS.len()
                        );
                    }
                }
            }
        }
        TurnColors {
            cycle_idx_per_turn: m,
        }
    }

    pub fn color_t(&self, t: &Turn) -> Option<Color> {
        if let Some(cycle) = self.cycle_idx_per_turn.get(&t.id) {
            return Some(CYCLE_COLORS[*cycle]);
        }
        None
    }
}

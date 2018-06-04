// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use CycleState;
use ModifiedTrafficSignal;

use dimensioned::si;
use geom::GeomMap;
use map_model::{IntersectionID, Map, TurnID};

use std;
const CYCLE_DURATION: si::Second<f64> = si::Second {
    value_unsafe: 15.0,
    _marker: std::marker::PhantomData,
};

#[derive(Debug)]
pub struct ControlTrafficSignal {
    intersection: IntersectionID,
    pub cycles: Vec<Cycle>,
}

impl ControlTrafficSignal {
    pub fn new(
        map: &Map,
        intersection: IntersectionID,
        geom_map: &GeomMap,
    ) -> ControlTrafficSignal {
        assert!(map.get_i(intersection).has_traffic_signal);
        ControlTrafficSignal {
            intersection,
            cycles: ControlTrafficSignal::greedy_assignment(map, intersection, geom_map),
        }
    }

    pub fn changed(&self) -> bool {
        self.cycles.iter().find(|c| c.changed).is_some()
    }

    pub fn get_savestate(&self) -> Option<ModifiedTrafficSignal> {
        if !self.changed() {
            return None;
        }
        Some(ModifiedTrafficSignal {
            cycles: self.cycles
                .iter()
                .map(|c| CycleState {
                    turns: c.turns.clone(),
                })
                .collect(),
        })
    }

    pub fn load_savestate(&mut self, state: &ModifiedTrafficSignal) {
        self.cycles = state
            .cycles
            .iter()
            .map(|c| Cycle {
                turns: c.turns.clone(),
                changed: true,
                duration: CYCLE_DURATION,
            })
            .collect();
    }

    pub fn current_cycle_and_remaining_time(
        &self,
        time: si::Second<f64>,
    ) -> (&Cycle, si::Second<f64>) {
        let cycle_idx = (time / CYCLE_DURATION).floor() as usize;
        let cycle = &self.cycles[cycle_idx % self.cycles.len()];
        let next_cycle_time = (cycle_idx + 1) as f64 * CYCLE_DURATION;
        let remaining_cycle_time = next_cycle_time - time;
        (cycle, remaining_cycle_time)
    }

    fn greedy_assignment(
        map: &Map,
        intersection: IntersectionID,
        geom_map: &GeomMap,
    ) -> Vec<Cycle> {
        let mut cycles = Vec::new();

        // Greedily partition turns into cycles. More clever things later.
        let mut remaining_turns: Vec<TurnID> = map.get_turns_in_intersection(intersection)
            .iter()
            .map(|t| t.id)
            .collect();
        let mut current_cycle = Cycle {
            turns: Vec::new(),
            changed: false,
            duration: CYCLE_DURATION,
        };
        while !remaining_turns.is_empty() {
            let add_turn = remaining_turns
                .iter()
                .position(|&t| !current_cycle.conflicts_with(t, geom_map));
            match add_turn {
                Some(idx) => {
                    current_cycle.turns.push(remaining_turns[idx]);
                    remaining_turns.remove(idx);
                }
                None => {
                    cycles.push(current_cycle.clone());
                    current_cycle.turns = Vec::new();
                }
            }
        }
        // TODO not sure this condition is needed
        if !current_cycle.turns.is_empty() {
            cycles.push(current_cycle.clone());
        }

        // TODO second pass to add all legal turns to the cycles we came up with initially

        cycles
    }
}

#[derive(Clone, Debug)]
pub struct Cycle {
    pub turns: Vec<TurnID>,
    // in the future, what pedestrian crossings this cycle includes, etc
    changed: bool,
    duration: si::Second<f64>,
}

impl Cycle {
    pub fn conflicts_with(&self, t1: TurnID, geom_map: &GeomMap) -> bool {
        for t2 in &self.turns {
            if geom_map.get_t(t1).conflicts_with(geom_map.get_t(*t2)) {
                return true;
            }
        }
        false
    }

    pub fn contains(&self, t: TurnID) -> bool {
        self.turns.contains(&t)
    }

    pub fn add(&mut self, t: TurnID) {
        // should assert parent matches, compatible with cycle so far, not in the current set
        self.turns.push(t);
        self.changed = true;
    }

    pub fn remove(&mut self, t: TurnID) {
        // should assert in current set
        let idx = self.turns.iter().position(|&id| id == t).unwrap();
        self.turns.remove(idx);
        self.changed = true;
    }
}

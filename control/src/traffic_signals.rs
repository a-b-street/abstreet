// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use map_model::{IntersectionID, Map, TurnID};
use std;

const CYCLE_DURATION: si::Second<f64> = si::Second {
    value_unsafe: 15.0,
    _marker: std::marker::PhantomData,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControlTrafficSignal {
    intersection: IntersectionID,
    pub cycles: Vec<Cycle>,
}

impl ControlTrafficSignal {
    pub fn new(map: &Map, intersection: IntersectionID) -> ControlTrafficSignal {
        assert!(map.get_i(intersection).has_traffic_signal);
        ControlTrafficSignal {
            intersection,
            cycles: greedy_assignment(map, intersection),
        }
    }

    pub fn is_changed(&self) -> bool {
        self.cycles.iter().find(|c| c.changed).is_some()
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cycle {
    pub turns: Vec<TurnID>,
    // in the future, what pedestrian crossings this cycle includes, etc
    changed: bool,
    duration: si::Second<f64>,
}

impl Cycle {
    pub fn conflicts_with(&self, t1: TurnID, map: &Map) -> bool {
        for t2 in &self.turns {
            if map.get_t(t1).conflicts_with(map.get_t(*t2)) {
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

fn greedy_assignment(map: &Map, intersection: IntersectionID) -> Vec<Cycle> {
    /*
    // TODO should be a tmp hack; intersections with no turns aren't even valid
    if map.get_turns_in_intersection(intersection).is_empty() {
        println!("WARNING: {} has no turns", intersection);
        return vec![Cycle {
            turns: Vec::new(),
            changed: false,
            duration: CYCLE_DURATION,
        }];
    }
    */

    let mut cycles = Vec::new();

    // Greedily partition turns into cycles. More clever things later.
    let mut remaining_turns: Vec<TurnID> = map
        .get_turns_in_intersection(intersection)
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
            .position(|&t| !current_cycle.conflicts_with(t, map));
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

    expand_all_cycles(&mut cycles, map, intersection);

    cycles
}

// Add all legal turns to existing cycles.
fn expand_all_cycles(cycles: &mut Vec<Cycle>, map: &Map, intersection: IntersectionID) {
    let all_turns: Vec<TurnID> = map
        .get_turns_in_intersection(intersection)
        .iter()
        .map(|t| t.id)
        .collect();
    for cycle in cycles.iter_mut() {
        for t in &all_turns {
            if !cycle.contains(*t) && !cycle.conflicts_with(*t, map) {
                cycle.turns.push(*t);
            }
        }
    }
}

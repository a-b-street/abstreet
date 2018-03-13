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

use map_model::{IntersectionID, Map, TurnID};
use render::DrawTurn;
use savestate;

#[derive(Debug)]
pub struct TrafficSignal {
    intersection: IntersectionID,
    pub cycles: Vec<Cycle>,
}

impl TrafficSignal {
    pub fn new(map: &Map, intersection: IntersectionID, turns: &[DrawTurn]) -> TrafficSignal {
        assert!(map.get_i(intersection).has_traffic_signal);
        TrafficSignal {
            intersection,
            cycles: TrafficSignal::greedy_assignment(map, intersection, turns),
        }
    }

    pub fn changed(&self) -> bool {
        self.cycles.iter().find(|c| c.changed).is_some()
    }

    pub fn get_savestate(&self) -> Option<savestate::ModifiedTrafficSignal> {
        if !self.changed() {
            return None;
        }
        Some(savestate::ModifiedTrafficSignal {
            cycles: self.cycles
                .iter()
                .map(|c| savestate::CycleState {
                    turns: c.turns.clone(),
                })
                .collect(),
        })
    }

    pub fn load_savestate(&mut self, state: &savestate::ModifiedTrafficSignal) {
        self.cycles = state
            .cycles
            .iter()
            .map(|c| Cycle {
                turns: c.turns.clone(),
                changed: true,
            })
            .collect();
    }

    fn greedy_assignment(
        map: &Map,
        intersection: IntersectionID,
        turns: &[DrawTurn],
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
        };
        while !remaining_turns.is_empty() {
            let add_turn = remaining_turns
                .iter()
                .position(|&t| !current_cycle.conflicts_with(t, turns));
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
    // in the future, the cycle time, what pedestrian crossings this cycle includes, etc
    changed: bool,
}

impl Cycle {
    pub fn conflicts_with(&self, t1: TurnID, turns: &[DrawTurn]) -> bool {
        for t2 in &self.turns {
            if turns[t1.0].conflicts_with(&turns[t2.0]) {
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

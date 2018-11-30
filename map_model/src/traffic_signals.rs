use dimensioned::si;
use std;
use std::collections::BTreeSet;
use {IntersectionID, Map, TurnID, TurnPriority};

const CYCLE_DURATION: si::Second<f64> = si::Second {
    value_unsafe: 15.0,
    _marker: std::marker::PhantomData,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ControlTrafficSignal {
    pub id: IntersectionID,
    pub cycles: Vec<Cycle>,
}

impl ControlTrafficSignal {
    pub fn new(map: &Map, id: IntersectionID) -> ControlTrafficSignal {
        ControlTrafficSignal {
            id,
            cycles: greedy_assignment(map, id),
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
    pub priority_turns: BTreeSet<TurnID>,
    pub yield_turns: BTreeSet<TurnID>,
    changed: bool,
    duration: si::Second<f64>,
}

impl Cycle {
    pub fn could_be_priority_turn(&self, t1: TurnID, map: &Map) -> bool {
        for t2 in &self.priority_turns {
            if map.get_t(t1).conflicts_with(map.get_t(*t2)) {
                return false;
            }
        }
        true
    }

    pub fn get_priority(&self, t: TurnID) -> TurnPriority {
        if self.priority_turns.contains(&t) {
            TurnPriority::Priority
        } else if self.yield_turns.contains(&t) {
            TurnPriority::Yield
        } else {
            TurnPriority::Stop
        }
    }

    pub fn add(&mut self, t: TurnID, pri: TurnPriority) {
        // should assert parent matches, compatible with cycle so far, not in the current set
        match pri {
            TurnPriority::Priority => {
                self.priority_turns.insert(t);
                self.changed = true;
            }
            TurnPriority::Yield => {
                self.yield_turns.insert(t);
                self.changed = true;
            }
            TurnPriority::Stop => {}
        }
    }

    pub fn remove(&mut self, t: TurnID) {
        if self.priority_turns.contains(&t) {
            self.priority_turns.remove(&t);
        } else if self.yield_turns.contains(&t) {
            self.yield_turns.remove(&t);
        } else {
            panic!(
                "Cycle {:?} doesn't have {} as a priority or yield turn; why remove it?",
                self, t
            );
        }
    }
}

fn greedy_assignment(map: &Map, intersection: IntersectionID) -> Vec<Cycle> {
    if map.get_turns_in_intersection(intersection).is_empty() {
        panic!("{} has no turns", intersection);
    }

    let mut cycles = Vec::new();

    // Greedily partition turns into cycles. More clever things later. No yields.
    let mut remaining_turns: Vec<TurnID> = map
        .get_turns_in_intersection(intersection)
        .iter()
        .map(|t| t.id)
        .collect();
    let mut current_cycle = Cycle {
        priority_turns: BTreeSet::new(),
        yield_turns: BTreeSet::new(),
        changed: false,
        duration: CYCLE_DURATION,
    };
    loop {
        let add_turn = remaining_turns
            .iter()
            .position(|&t| current_cycle.could_be_priority_turn(t, map));
        match add_turn {
            Some(idx) => {
                current_cycle
                    .priority_turns
                    .insert(remaining_turns.remove(idx));
            }
            None => {
                cycles.push(current_cycle.clone());
                current_cycle.priority_turns.clear();
                if remaining_turns.is_empty() {
                    break;
                }
            }
        }
    }

    expand_all_cycles(&mut cycles, map, intersection);

    cycles
}

// Add all legal priority turns to existing cycles.
fn expand_all_cycles(cycles: &mut Vec<Cycle>, map: &Map, intersection: IntersectionID) {
    let all_turns: Vec<TurnID> = map
        .get_turns_in_intersection(intersection)
        .iter()
        .map(|t| t.id)
        .collect();
    for cycle in cycles.iter_mut() {
        for t in &all_turns {
            if !cycle.priority_turns.contains(t) && cycle.could_be_priority_turn(*t, map) {
                cycle.priority_turns.insert(*t);
            }
        }
    }
}

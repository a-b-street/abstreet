use abstutil::Error;
use dimensioned::si;
use std;
use std::collections::BTreeSet;
use {IntersectionID, Map, RoadID, TurnID, TurnPriority, TurnType};

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
        let ts = smart_assignment(map, id);
        ts.validate(map).unwrap();
        ts
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

    fn validate(&self, map: &Map) -> Result<(), Error> {
        // Does the assignment cover the correct set of turns?
        let expected_turns: BTreeSet<TurnID> = map.get_i(self.id).turns.iter().cloned().collect();
        let mut actual_turns: BTreeSet<TurnID> = BTreeSet::new();
        for cycle in &self.cycles {
            actual_turns.extend(cycle.priority_turns.clone());
            actual_turns.extend(cycle.yield_turns.clone());
        }
        if expected_turns != actual_turns {
            return Err(Error::new(format!("Traffic signal assignment for {} broken. Missing turns {:?}, contains irrelevant turns {:?}", self.id, expected_turns.difference(&actual_turns), actual_turns.difference(&expected_turns))));
        }

        // Do any of the priority turns in one cycle conflict?
        for cycle in &self.cycles {
            for t1 in cycle.priority_turns.iter().map(|t| map.get_t(*t)) {
                for t2 in cycle.priority_turns.iter().map(|t| map.get_t(*t)) {
                    if t1.conflicts_with(t2) {
                        return Err(Error::new(format!(
                            "Traffic signal has conflicting priority turns in one cycle:\n{:?}\n\n{:?}",
                            t1, t2
                        )));
                    }
                }
            }
        }

        Ok(())
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

fn greedy_assignment(map: &Map, intersection: IntersectionID) -> ControlTrafficSignal {
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

    ControlTrafficSignal {
        id: intersection,
        cycles,
    }
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

fn smart_assignment(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    let num_roads = map.get_i(i).roads.len();
    let ts = if num_roads == 3 {
        three_way(map, i)
    } else if num_roads == 4 {
        four_way(map, i)
    } else {
        return greedy_assignment(map, i);
    };

    match ts.validate(map) {
        Ok(()) => ts,
        Err(err) => {
            warn!("For {}: {}", i, err);
            greedy_assignment(map, i)
        }
    }
}

const PROTECTED: bool = true;
const YIELD: bool = false;

fn four_way(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
    let roads = map.get_i(i).get_roads_sorted_by_incoming_angle(map);
    let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

    // Two-phase with no protected lefts, right turn on red, peds yielding to cars
    /*let cycles = make_cycles(
        map,
        i,
        vec![
            vec![
                (vec![north, south], Turns::StraightAndRight, PROTECTED),
                (vec![north, south], Turns::Left, YIELD),
                (vec![east, west], Turns::Right, YIELD),
                (vec![east, west], Turns::Crosswalk, YIELD),
            ],
            vec![
                (vec![east, west], Turns::StraightAndRight, PROTECTED),
                (vec![east, west], Turns::Left, YIELD),
                (vec![north, south], Turns::Right, YIELD),
                (vec![north, south], Turns::Crosswalk, YIELD),
            ],
        ],
    );*/

    // Four-phase with protected lefts, right turn on red (except for the protected lefts), peds
    // yielding to cars
    let cycles = make_cycles(
        map,
        i,
        vec![
            vec![
                (vec![north, south], Turns::StraightAndRight, PROTECTED),
                (vec![east, west], Turns::Right, YIELD),
                (vec![east, west], Turns::Crosswalk, YIELD),
            ],
            vec![(vec![north, south], Turns::Left, PROTECTED)],
            vec![
                (vec![east, west], Turns::StraightAndRight, PROTECTED),
                (vec![north, south], Turns::Right, YIELD),
                (vec![north, south], Turns::Crosswalk, YIELD),
            ],
            vec![(vec![east, west], Turns::Left, PROTECTED)],
        ],
    );

    ControlTrafficSignal { id: i, cycles }
}

fn three_way(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    // Picture a T intersection. Use turn angles to figure out the "main" two roads.
    let straight_turn = map
        .get_turns_in_intersection(i)
        .into_iter()
        .find(|t| t.turn_type == TurnType::Straight)
        .unwrap();
    let (north, south) = (
        map.get_l(straight_turn.id.src).parent,
        map.get_l(straight_turn.id.dst).parent,
    );
    let mut roads = map.get_i(i).roads.clone();
    roads.remove(&north);
    roads.remove(&south);
    let east = roads.into_iter().next().unwrap();

    // Two-phase with no protected lefts, right turn on red, peds yielding to cars
    let cycles = make_cycles(
        map,
        i,
        vec![
            vec![
                (vec![north, south], Turns::StraightAndRight, PROTECTED),
                (vec![north, south], Turns::Left, YIELD),
                (vec![east], Turns::Right, YIELD),
                (vec![east], Turns::Crosswalk, YIELD),
            ],
            vec![
                (vec![east], Turns::StraightAndRight, PROTECTED),
                (vec![east], Turns::Left, PROTECTED),
                (vec![north, south], Turns::Right, YIELD),
                (vec![north, south], Turns::Crosswalk, YIELD),
            ],
        ],
    );

    ControlTrafficSignal { id: i, cycles }
}

fn make_cycles(
    map: &Map,
    i: IntersectionID,
    cycle_specs: Vec<Vec<(Vec<RoadID>, Turns, bool)>>,
) -> Vec<Cycle> {
    let mut cycles: Vec<Cycle> = Vec::new();

    for specs in cycle_specs.into_iter() {
        let mut cycle = Cycle {
            priority_turns: BTreeSet::new(),
            yield_turns: BTreeSet::new(),
            changed: false,
            duration: CYCLE_DURATION,
        };

        for (roads, turns, protected) in specs.into_iter() {
            for turn in map.get_turns_in_intersection(i) {
                // These never conflict with anything.
                if turn.turn_type == TurnType::SharedSidewalkCorner {
                    cycle.priority_turns.insert(turn.id);
                    continue;
                }

                if !roads.contains(&map.get_l(turn.id.src).parent) {
                    continue;
                }

                match turn.turn_type {
                    TurnType::Crosswalk => if turns != Turns::Crosswalk {
                        continue;
                    },
                    TurnType::Straight => if turns != Turns::StraightAndRight {
                        continue;
                    },
                    TurnType::Right => if turns != Turns::StraightAndRight && turns != Turns::Right
                    {
                        continue;
                    },
                    TurnType::Left => if turns != Turns::Left {
                        continue;
                    },
                    TurnType::SharedSidewalkCorner => unreachable!(),
                };

                if protected {
                    cycle.priority_turns.insert(turn.id);
                } else {
                    cycle.yield_turns.insert(turn.id);
                }
            }
        }

        cycles.push(cycle);
    }

    cycles
}

#[derive(PartialEq)]
enum Turns {
    StraightAndRight,
    Left,
    Right,
    // These always cross from one side of a road to the other.
    Crosswalk,
}

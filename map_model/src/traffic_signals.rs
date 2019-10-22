use crate::{IntersectionID, Map, RoadID, Turn, TurnID, TurnPriority, TurnType};
use abstutil::{Error, Timer};
use geom::Duration;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeSet;

const PHASE_DURATION: Duration = Duration::const_seconds(30.0);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ControlTrafficSignal {
    pub id: IntersectionID,
    pub phases: Vec<Phase>,
}

impl ControlTrafficSignal {
    pub fn new(map: &Map, id: IntersectionID, timer: &mut Timer) -> ControlTrafficSignal {
        let mut policies = ControlTrafficSignal::get_possible_policies(map, id);
        if policies.len() == 1 {
            timer.warn(format!("Falling back to greedy_assignment for {}", id));
        }
        policies.remove(0).1
    }

    pub fn get_possible_policies(
        map: &Map,
        id: IntersectionID,
    ) -> Vec<(String, ControlTrafficSignal)> {
        let mut results = Vec::new();
        if let Some(ts) = ControlTrafficSignal::four_way_four_phase(map, id) {
            results.push(("four-phase".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::four_way_two_phase(map, id) {
            results.push(("two-phase".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::three_way(map, id) {
            results.push(("three-phase".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::degenerate(map, id) {
            results.push(("degenerate (2 roads)".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::four_oneways(map, id) {
            results.push(("two-phase for 4 one-ways".to_string(), ts));
        }
        results.push((
            "arbitrary assignment".to_string(),
            ControlTrafficSignal::greedy_assignment(map, id),
        ));
        results.push((
            "all walk, then free-for-all yield".to_string(),
            ControlTrafficSignal::all_walk_all_yield(map, id),
        ));
        results
    }

    pub fn current_phase_and_remaining_time(&self, now: Duration) -> (usize, &Phase, Duration) {
        let phase_idx = (now / PHASE_DURATION).floor() as usize;
        let normalized_phase_idx = phase_idx % self.phases.len();
        let phase = &self.phases[normalized_phase_idx];
        let next_phase_time = PHASE_DURATION * (phase_idx + 1) as f64;
        let remaining_phase_time = next_phase_time - now;
        (normalized_phase_idx, phase, remaining_phase_time)
    }

    fn validate(self, map: &Map) -> Result<ControlTrafficSignal, Error> {
        // TODO Reuse assertions from edit_turn.

        // Does the assignment cover the correct set of turns?
        let expected_turns: BTreeSet<TurnID> = map.get_i(self.id).turns.iter().cloned().collect();
        let mut actual_turns: BTreeSet<TurnID> = BTreeSet::new();
        for phase in &self.phases {
            actual_turns.extend(phase.priority_turns.iter());
            actual_turns.extend(phase.yield_turns.iter());
        }
        if expected_turns != actual_turns {
            return Err(Error::new(format!("Traffic signal assignment for {} broken. Missing turns {:?}, contains irrelevant turns {:?}", self.id, expected_turns.difference(&actual_turns).cloned().collect::<Vec<TurnID>>(), actual_turns.difference(&expected_turns).cloned().collect::<Vec<TurnID>>())));
        }

        for phase in &self.phases {
            // Do any of the priority turns in one phase conflict?
            for t1 in phase.priority_turns.iter().map(|t| map.get_t(*t)) {
                for t2 in phase.priority_turns.iter().map(|t| map.get_t(*t)) {
                    if t1.conflicts_with(t2) {
                        return Err(Error::new(format!(
                            "Traffic signal has conflicting priority turns in one phase:\n{:?}\n\n{:?}",
                            t1, t2
                        )));
                    }
                }
            }

            // Do any of the crosswalks yield? Are all of the SharedSidewalkCorner prioritized?
            for t in map.get_turns_in_intersection(self.id) {
                match t.turn_type {
                    TurnType::Crosswalk => {
                        assert!(!phase.yield_turns.contains(&t.id));
                    }
                    TurnType::SharedSidewalkCorner => {
                        assert!(phase.priority_turns.contains(&t.id));
                    }
                    _ => {}
                }
            }
        }

        Ok(self)
    }

    fn greedy_assignment(map: &Map, intersection: IntersectionID) -> ControlTrafficSignal {
        if map.get_turns_in_intersection(intersection).is_empty() {
            panic!("{} has no turns", intersection);
        }

        let mut phases = Vec::new();

        // Greedily partition turns into phases. More clever things later. No yields.
        let mut remaining_turns: Vec<TurnID> = map
            .get_turns_in_intersection(intersection)
            .iter()
            .map(|t| t.id)
            .collect();
        let mut current_phase = Phase::new(intersection);
        loop {
            let add_turn = remaining_turns
                .iter()
                .position(|&t| current_phase.could_be_priority_turn(t, map));
            match add_turn {
                Some(idx) => {
                    current_phase
                        .priority_turns
                        .insert(remaining_turns.remove(idx));
                }
                None => {
                    phases.push(current_phase);
                    current_phase = Phase::new(intersection);
                    if remaining_turns.is_empty() {
                        break;
                    }
                }
            }
        }

        expand_all_phases(&mut phases, map, intersection);

        let ts = ControlTrafficSignal {
            id: intersection,
            phases,
        };
        // This must succeed
        ts.validate(map).unwrap()
    }

    fn degenerate(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
        if map.get_i(i).roads.len() != 2 {
            return None;
        }

        let mut roads = map.get_i(i).roads.iter();
        let r1 = *roads.next().unwrap();
        let r2 = *roads.next().unwrap();
        // TODO One-ways downtown should also have crosswalks.
        let has_crosswalks = !map.get_r(r1).children_backwards.is_empty()
            || !map.get_r(r2).children_backwards.is_empty();
        let mut phases = vec![vec![
            (vec![r1, r2], TurnType::Straight, PROTECTED),
            (vec![r1, r2], TurnType::LaneChangeLeft, YIELD),
            (vec![r1, r2], TurnType::LaneChangeRight, YIELD),
        ]];
        if has_crosswalks {
            phases.push(vec![(vec![r1, r2], TurnType::Crosswalk, PROTECTED)]);
        }

        let phases = make_phases(map, i, phases);

        let ts = ControlTrafficSignal { id: i, phases };
        ts.validate(map).ok()
    }

    fn three_way(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
        if map.get_i(i).roads.len() != 3 {
            return None;
        }

        // Picture a T intersection. Use turn angles to figure out the "main" two roads.
        let straight_turn = map
            .get_turns_in_intersection(i)
            .into_iter()
            .find(|t| t.turn_type == TurnType::Straight)?;
        let (north, south) = (
            map.get_l(straight_turn.id.src).parent,
            map.get_l(straight_turn.id.dst).parent,
        );
        let mut roads = map.get_i(i).roads.clone();
        roads.remove(&north);
        roads.remove(&south);
        let east = roads.into_iter().next().unwrap();

        // Two-phase with no protected lefts, right turn on red, turning cars yield to peds
        let phases = make_phases(
            map,
            i,
            vec![
                vec![
                    (vec![north, south], TurnType::Straight, PROTECTED),
                    (vec![north, south], TurnType::LaneChangeLeft, YIELD),
                    (vec![north, south], TurnType::LaneChangeRight, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Left, YIELD),
                    (vec![east], TurnType::Right, YIELD),
                    (vec![east], TurnType::Crosswalk, PROTECTED),
                ],
                vec![
                    (vec![east], TurnType::Straight, PROTECTED),
                    (vec![east], TurnType::LaneChangeLeft, YIELD),
                    (vec![east], TurnType::LaneChangeRight, YIELD),
                    (vec![east], TurnType::Right, YIELD),
                    (vec![east], TurnType::Left, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Crosswalk, PROTECTED),
                ],
            ],
        );

        let ts = ControlTrafficSignal { id: i, phases };
        ts.validate(map).ok()
    }

    fn four_way_four_phase(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
        if map.get_i(i).roads.len() != 4 {
            return None;
        }

        // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
        let roads = map
            .get_i(i)
            .get_roads_sorted_by_incoming_angle(map.all_roads());
        let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

        // Four-phase with protected lefts, right turn on red (except for the protected lefts), turning
        // cars yield to peds
        let phases = make_phases(
            map,
            i,
            vec![
                vec![
                    (vec![north, south], TurnType::Straight, PROTECTED),
                    (vec![north, south], TurnType::LaneChangeLeft, YIELD),
                    (vec![north, south], TurnType::LaneChangeRight, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Crosswalk, PROTECTED),
                ],
                vec![(vec![north, south], TurnType::Left, PROTECTED)],
                vec![
                    (vec![east, west], TurnType::Straight, PROTECTED),
                    (vec![east, west], TurnType::LaneChangeLeft, YIELD),
                    (vec![east, west], TurnType::LaneChangeRight, YIELD),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Crosswalk, PROTECTED),
                ],
                vec![(vec![east, west], TurnType::Left, PROTECTED)],
            ],
        );

        let ts = ControlTrafficSignal { id: i, phases };
        ts.validate(map).ok()
    }

    fn four_way_two_phase(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
        if map.get_i(i).roads.len() != 4 {
            return None;
        }

        // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
        let roads = map
            .get_i(i)
            .get_roads_sorted_by_incoming_angle(map.all_roads());
        let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

        // Two-phase with no protected lefts, right turn on red, turning cars yielding to peds
        let phases = make_phases(
            map,
            i,
            vec![
                vec![
                    (vec![north, south], TurnType::Straight, PROTECTED),
                    (vec![north, south], TurnType::LaneChangeLeft, YIELD),
                    (vec![north, south], TurnType::LaneChangeRight, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Left, YIELD),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Crosswalk, PROTECTED),
                ],
                vec![
                    (vec![east, west], TurnType::Straight, PROTECTED),
                    (vec![east, west], TurnType::LaneChangeLeft, YIELD),
                    (vec![east, west], TurnType::LaneChangeRight, YIELD),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Left, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Crosswalk, PROTECTED),
                ],
            ],
        );

        let ts = ControlTrafficSignal { id: i, phases };
        ts.validate(map).ok()
    }

    fn four_oneways(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
        if map.get_i(i).roads.len() != 4 {
            return None;
        }

        let mut incomings = Vec::new();
        for r in &map.get_i(i).roads {
            if !map.get_r(*r).incoming_lanes(i).is_empty() {
                incomings.push(*r);
            }
        }
        if incomings.len() != 2 {
            return None;
        }
        let r1 = incomings[0];
        let r2 = incomings[1];

        // TODO This may not generalize...
        let phases = make_phases(
            map,
            i,
            vec![
                vec![
                    (vec![r1], TurnType::Straight, PROTECTED),
                    (vec![r1], TurnType::LaneChangeLeft, YIELD),
                    (vec![r1], TurnType::LaneChangeRight, YIELD),
                    (vec![r1], TurnType::Crosswalk, PROTECTED),
                    // TODO Technically, upgrade to protected if there's no opposing crosswalk --
                    // even though it doesn't matter much.
                    (vec![r1], TurnType::Right, YIELD),
                    (vec![r1], TurnType::Left, YIELD),
                    (vec![r1], TurnType::Right, YIELD),
                    // TODO Refactor
                ],
                vec![
                    (vec![r2], TurnType::Straight, PROTECTED),
                    (vec![r2], TurnType::LaneChangeLeft, YIELD),
                    (vec![r2], TurnType::LaneChangeRight, YIELD),
                    (vec![r2], TurnType::Crosswalk, PROTECTED),
                    // TODO Technically, upgrade to protected if there's no opposing crosswalk --
                    // even though it doesn't matter much.
                    (vec![r2], TurnType::Right, YIELD),
                    (vec![r2], TurnType::Left, YIELD),
                    (vec![r2], TurnType::Right, YIELD),
                ],
            ],
        );

        let ts = ControlTrafficSignal { id: i, phases };
        ts.validate(map).ok()
    }

    fn all_walk_all_yield(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
        let mut all_walk = Phase::new(i);
        let mut all_yield = Phase::new(i);

        for turn in map.get_turns_in_intersection(i) {
            match turn.turn_type {
                TurnType::SharedSidewalkCorner => {
                    all_walk.priority_turns.insert(turn.id);
                    all_yield.priority_turns.insert(turn.id);
                }
                TurnType::Crosswalk => {
                    all_walk.priority_turns.insert(turn.id);
                }
                _ => {
                    all_yield.yield_turns.insert(turn.id);
                }
            }
        }

        let ts = ControlTrafficSignal {
            id: i,
            phases: vec![all_walk, all_yield],
        };
        // This must succeed
        ts.validate(map).unwrap()
    }

    pub fn convert_to_ped_scramble(&mut self, map: &Map) {
        // Remove Crosswalk turns from existing phases.
        for phase in self.phases.iter_mut() {
            // Crosswalks are usually only priority_turns, but also clear out from yield_turns.
            for t in map.get_turns_in_intersection(self.id) {
                if t.turn_type == TurnType::Crosswalk {
                    phase.priority_turns.remove(&t.id);
                    phase.yield_turns.remove(&t.id);
                }
            }

            // Blindly try to promote yield turns to protected, now that crosswalks are gone.
            let mut promoted = Vec::new();
            for t in &phase.yield_turns {
                if phase.could_be_priority_turn(*t, map) {
                    phase.priority_turns.insert(*t);
                    promoted.push(*t);
                }
            }
            for t in promoted {
                phase.yield_turns.remove(&t);
            }
        }

        let mut phase = Phase::new(self.id);
        for t in map.get_turns_in_intersection(self.id) {
            if t.between_sidewalks() {
                phase.edit_turn(t, TurnPriority::Priority);
            }
        }
        self.phases.push(phase);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Phase {
    pub parent: IntersectionID,
    pub priority_turns: BTreeSet<TurnID>,
    pub yield_turns: BTreeSet<TurnID>,
    pub duration: Duration,
}

impl Phase {
    pub fn new(parent: IntersectionID) -> Phase {
        Phase {
            parent,
            priority_turns: BTreeSet::new(),
            yield_turns: BTreeSet::new(),
            duration: PHASE_DURATION,
        }
    }

    pub fn could_be_priority_turn(&self, t1: TurnID, map: &Map) -> bool {
        let turn1 = map.get_t(t1);
        for t2 in &self.priority_turns {
            if t1 == *t2 || turn1.conflicts_with(map.get_t(*t2)) {
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
            TurnPriority::Banned
        }
    }

    pub fn edit_turn(&mut self, t: &Turn, pri: TurnPriority) {
        let mut ids = vec![t.id];
        if t.turn_type == TurnType::Crosswalk {
            ids.extend(t.other_crosswalk_ids.clone());
        }
        for id in ids {
            self.priority_turns.remove(&id);
            self.yield_turns.remove(&id);
            if pri == TurnPriority::Priority {
                self.priority_turns.insert(id);
            } else if pri == TurnPriority::Yield {
                self.yield_turns.insert(id);
            }
        }
    }
}

// Add all legal priority turns to existing phases.
fn expand_all_phases(phases: &mut Vec<Phase>, map: &Map, intersection: IntersectionID) {
    let all_turns: Vec<TurnID> = map
        .get_turns_in_intersection(intersection)
        .iter()
        .map(|t| t.id)
        .collect();
    for phase in phases.iter_mut() {
        for t in &all_turns {
            if !phase.priority_turns.contains(t) && phase.could_be_priority_turn(*t, map) {
                phase.priority_turns.insert(*t);
            }
        }
    }
}

const PROTECTED: bool = true;
const YIELD: bool = false;

fn make_phases(
    map: &Map,
    i: IntersectionID,
    phase_specs: Vec<Vec<(Vec<RoadID>, TurnType, bool)>>,
) -> Vec<Phase> {
    let mut phases: Vec<Phase> = Vec::new();

    for specs in phase_specs {
        let mut phase = Phase::new(i);
        let mut empty = true;

        for (roads, turn_type, protected) in specs.into_iter() {
            for turn in map.get_turns_in_intersection(i) {
                // These never conflict with anything.
                if turn.turn_type == TurnType::SharedSidewalkCorner {
                    phase.priority_turns.insert(turn.id);
                    continue;
                }

                if !roads.contains(&map.get_l(turn.id.src).parent) || turn_type != turn.turn_type {
                    continue;
                }

                phase.edit_turn(
                    turn,
                    if protected {
                        TurnPriority::Priority
                    } else {
                        TurnPriority::Yield
                    },
                );
                empty = false;
            }
        }

        // Filter out empty phases if they happen.
        if empty {
            continue;
        }

        phases.push(phase);
    }

    phases
}

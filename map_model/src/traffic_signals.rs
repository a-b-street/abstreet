use crate::{
    DirectedRoadID, IntersectionID, Map, RoadID, TurnGroup, TurnGroupID, TurnID, TurnPriority,
    TurnType,
};
use abstutil::{deserialize_btreemap, retain_btreeset, serialize_btreemap, Timer};
use geom::{Duration, Time};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ControlTrafficSignal {
    pub id: IntersectionID,
    pub phases: Vec<Phase>,
    pub offset: Duration,

    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub turn_groups: BTreeMap<TurnGroupID, TurnGroup>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Phase {
    pub protected_groups: BTreeSet<TurnGroupID>,
    pub yield_groups: BTreeSet<TurnGroupID>,
    pub duration: Duration,
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

        // TODO Cache with lazy_static. Don't serialize in Map; the repo of signal data may evolve
        // independently.
        if let Some(raw) = seattle_traffic_signals::load_all_data()
            .unwrap()
            .remove(&map.get_i(id).orig_id.osm_node_id)
        {
            if let Some(ts) = ControlTrafficSignal::import(raw, id, map) {
                results.push(("hand-mapped current real settings".to_string(), ts));
            } else {
                panic!(
                    "seattle_traffic_signals data for {} out of date, go update it",
                    map.get_i(id).orig_id.osm_node_id
                );
            }
        }

        // As long as we're using silly heuristics for these by default, prefer shorter cycle
        // length.
        if let Some(ts) = ControlTrafficSignal::four_way_two_phase(map, id) {
            results.push(("two-phase".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::three_way(map, id) {
            results.push(("three-phase".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::four_way_four_phase(map, id) {
            results.push(("four-phase".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::degenerate(map, id) {
            results.push(("degenerate (2 roads)".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::four_oneways(map, id) {
            results.push(("two-phase for 4 one-ways".to_string(), ts));
        }
        if let Some(ts) = ControlTrafficSignal::phase_per_road(map, id) {
            results.push(("phase per road".to_string(), ts));
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

    pub fn cycle_length(&self) -> Duration {
        let mut cycle_length = Duration::ZERO;
        for p in &self.phases {
            cycle_length += p.duration;
        }
        cycle_length
    }

    pub fn current_phase_and_remaining_time(&self, now: Time) -> (usize, &Phase, Duration) {
        let mut now_offset = ((now + self.offset) - Time::START_OF_DAY) % self.cycle_length();
        for (idx, p) in self.phases.iter().enumerate() {
            if now_offset < p.duration {
                return (idx, p, p.duration - now_offset);
            } else {
                now_offset -= p.duration;
            }
        }
        unreachable!()
    }

    pub fn validate(self) -> Result<ControlTrafficSignal, String> {
        // Does the assignment cover the correct set of groups?
        let expected_groups: BTreeSet<TurnGroupID> = self.turn_groups.keys().cloned().collect();
        let mut actual_groups: BTreeSet<TurnGroupID> = BTreeSet::new();
        for phase in &self.phases {
            actual_groups.extend(phase.protected_groups.iter());
            actual_groups.extend(phase.yield_groups.iter());
        }
        if expected_groups != actual_groups {
            return Err(format!(
                "Traffic signal assignment for {} broken. Missing {:?}, contains irrelevant {:?}",
                self.id,
                expected_groups
                    .difference(&actual_groups)
                    .cloned()
                    .collect::<Vec<_>>(),
                actual_groups
                    .difference(&expected_groups)
                    .cloned()
                    .collect::<Vec<_>>()
            ));
        }

        for phase in &self.phases {
            // Do any of the priority groups in one phase conflict?
            for g1 in phase.protected_groups.iter().map(|g| &self.turn_groups[g]) {
                for g2 in phase.protected_groups.iter().map(|g| &self.turn_groups[g]) {
                    if g1.conflicts_with(g2) {
                        return Err(format!(
                            "Traffic signal has conflicting protected groups in one \
                             phase:\n{:?}\n\n{:?}",
                            g1, g2
                        ));
                    }
                }
            }

            // Do any of the crosswalks yield?
            for g in phase.yield_groups.iter().map(|g| &self.turn_groups[g]) {
                assert!(g.turn_type != TurnType::Crosswalk);
            }
        }

        Ok(self)
    }

    fn greedy_assignment(map: &Map, intersection: IntersectionID) -> ControlTrafficSignal {
        let turn_groups = TurnGroup::for_i(intersection, map);

        let mut phases = Vec::new();

        // Greedily partition groups into phases that only have protected groups.
        let mut remaining_groups: Vec<TurnGroupID> = turn_groups.keys().cloned().collect();
        let mut current_phase = Phase::new();
        loop {
            let add = remaining_groups
                .iter()
                .position(|&g| current_phase.could_be_protected(g, &turn_groups));
            match add {
                Some(idx) => {
                    current_phase
                        .protected_groups
                        .insert(remaining_groups.remove(idx));
                }
                None => {
                    assert!(!current_phase.protected_groups.is_empty());
                    phases.push(current_phase);
                    current_phase = Phase::new();
                    if remaining_groups.is_empty() {
                        break;
                    }
                }
            }
        }

        expand_all_phases(&mut phases, &turn_groups);

        let ts = ControlTrafficSignal {
            id: intersection,
            phases,
            offset: Duration::ZERO,
            turn_groups,
        };
        // This must succeed
        ts.validate().unwrap()
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
        let mut phases = vec![vec![(vec![r1, r2], TurnType::Straight, PROTECTED)]];
        if has_crosswalks {
            phases.push(vec![(vec![r1, r2], TurnType::Crosswalk, PROTECTED)]);
        }

        let phases = make_phases(map, i, phases);

        let ts = ControlTrafficSignal {
            id: i,
            phases,
            offset: Duration::ZERO,
            turn_groups: TurnGroup::for_i(i, map),
        };
        ts.validate().ok()
    }

    fn three_way(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
        if map.get_i(i).roads.len() != 3 {
            return None;
        }
        let turn_groups = TurnGroup::for_i(i, map);

        // Picture a T intersection. Use turn angles to figure out the "main" two roads.
        let straight = turn_groups
            .values()
            .find(|g| g.turn_type == TurnType::Straight)?;
        let (north, south) = (straight.id.from.id, straight.id.to.id);
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
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Left, YIELD),
                    (vec![east], TurnType::Right, YIELD),
                    (vec![east], TurnType::Crosswalk, PROTECTED),
                ],
                vec![
                    (vec![east], TurnType::Straight, PROTECTED),
                    (vec![east], TurnType::Right, YIELD),
                    (vec![east], TurnType::Left, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Crosswalk, PROTECTED),
                ],
            ],
        );

        let ts = ControlTrafficSignal {
            id: i,
            phases,
            offset: Duration::ZERO,
            turn_groups,
        };
        ts.validate().ok()
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

        // Four-phase with protected lefts, right turn on red (except for the protected lefts),
        // turning cars yield to peds
        let phases = make_phases(
            map,
            i,
            vec![
                vec![
                    (vec![north, south], TurnType::Straight, PROTECTED),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Crosswalk, PROTECTED),
                ],
                vec![(vec![north, south], TurnType::Left, PROTECTED)],
                vec![
                    (vec![east, west], TurnType::Straight, PROTECTED),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Crosswalk, PROTECTED),
                ],
                vec![(vec![east, west], TurnType::Left, PROTECTED)],
            ],
        );

        let ts = ControlTrafficSignal {
            id: i,
            phases,
            offset: Duration::ZERO,
            turn_groups: TurnGroup::for_i(i, map),
        };
        ts.validate().ok()
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
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Left, YIELD),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Crosswalk, PROTECTED),
                ],
                vec![
                    (vec![east, west], TurnType::Straight, PROTECTED),
                    (vec![east, west], TurnType::Right, YIELD),
                    (vec![east, west], TurnType::Left, YIELD),
                    (vec![north, south], TurnType::Right, YIELD),
                    (vec![north, south], TurnType::Crosswalk, PROTECTED),
                ],
            ],
        );

        let ts = ControlTrafficSignal {
            id: i,
            phases,
            offset: Duration::ZERO,
            turn_groups: TurnGroup::for_i(i, map),
        };
        ts.validate().ok()
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
                    (vec![r2], TurnType::Crosswalk, PROTECTED),
                    // TODO Technically, upgrade to protected if there's no opposing crosswalk --
                    // even though it doesn't matter much.
                    (vec![r2], TurnType::Right, YIELD),
                    (vec![r2], TurnType::Left, YIELD),
                    (vec![r2], TurnType::Right, YIELD),
                ],
            ],
        );

        let ts = ControlTrafficSignal {
            id: i,
            phases,
            offset: Duration::ZERO,
            turn_groups: TurnGroup::for_i(i, map),
        };
        ts.validate().ok()
    }

    fn all_walk_all_yield(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
        let turn_groups = TurnGroup::for_i(i, map);

        let mut all_walk = Phase::new();
        let mut all_yield = Phase::new();

        for group in turn_groups.values() {
            match group.turn_type {
                TurnType::Crosswalk => {
                    all_walk.protected_groups.insert(group.id);
                }
                _ => {
                    all_yield.yield_groups.insert(group.id);
                }
            }
        }

        let ts = ControlTrafficSignal {
            id: i,
            phases: vec![all_walk, all_yield],
            offset: Duration::ZERO,
            turn_groups,
        };
        // This must succeed
        ts.validate().unwrap()
    }

    fn phase_per_road(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
        let turn_groups = TurnGroup::for_i(i, map);

        let mut phases = Vec::new();
        let sorted_roads = map
            .get_i(i)
            .get_roads_sorted_by_incoming_angle(map.all_roads());
        for idx in 0..sorted_roads.len() {
            let r = sorted_roads[idx];
            let adj1 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) - 1);
            let adj2 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) + 1);

            let mut phase = Phase::new();
            for group in turn_groups.values() {
                if group.turn_type == TurnType::Crosswalk {
                    if group.id.from.id == adj1 || group.id.from.id == adj2 {
                        phase.protected_groups.insert(group.id);
                    }
                } else if group.id.from.id == r {
                    phase.yield_groups.insert(group.id);
                }
            }
            // Might have a one-way outgoing road. Skip it.
            if !phase.yield_groups.is_empty() {
                phases.push(phase);
            }
        }
        let ts = ControlTrafficSignal {
            id: i,
            phases,
            offset: Duration::ZERO,
            turn_groups,
        };
        ts.validate().ok()
    }

    // Returns true if this did anything
    pub fn convert_to_ped_scramble(&mut self) -> bool {
        let orig = self.clone();

        let mut all_walk_phase = Phase::new();
        for g in self.turn_groups.values() {
            if g.turn_type == TurnType::Crosswalk {
                all_walk_phase.edit_group(g, TurnPriority::Protected);
            }
        }

        // Remove Crosswalk groups from existing phases.
        let mut replaced = std::mem::replace(&mut self.phases, Vec::new());
        let mut has_all_walk = false;
        for phase in replaced.iter_mut() {
            if !has_all_walk && phase == &all_walk_phase {
                has_all_walk = true;
                continue;
            }

            // Crosswalks are only in protected_groups.
            retain_btreeset(&mut phase.protected_groups, |g| {
                self.turn_groups[g].turn_type != TurnType::Crosswalk
            });

            // Blindly try to promote yield groups to protected, now that crosswalks are gone.
            let mut promoted = Vec::new();
            for g in &phase.yield_groups {
                if phase.could_be_protected(*g, &self.turn_groups) {
                    phase.protected_groups.insert(*g);
                    promoted.push(*g);
                }
            }
            for g in promoted {
                phase.yield_groups.remove(&g);
            }
        }
        self.phases = replaced;

        if !has_all_walk {
            self.phases.push(all_walk_phase);
        }
        self != &orig
    }
}

impl Phase {
    pub fn new() -> Phase {
        Phase {
            protected_groups: BTreeSet::new(),
            yield_groups: BTreeSet::new(),
            duration: Duration::seconds(30.0),
        }
    }

    pub fn could_be_protected(
        &self,
        g1: TurnGroupID,
        turn_groups: &BTreeMap<TurnGroupID, TurnGroup>,
    ) -> bool {
        let group1 = &turn_groups[&g1];
        for g2 in &self.protected_groups {
            if g1 == *g2 || group1.conflicts_with(&turn_groups[g2]) {
                return false;
            }
        }
        true
    }

    pub fn get_priority_of_turn(&self, t: TurnID, parent: &ControlTrafficSignal) -> TurnPriority {
        // TODO Cache this?
        let g = parent
            .turn_groups
            .values()
            .find(|g| g.members.contains(&t))
            .map(|g| g.id)
            .unwrap();
        self.get_priority_of_group(g)
    }

    pub fn get_priority_of_group(&self, g: TurnGroupID) -> TurnPriority {
        if self.protected_groups.contains(&g) {
            TurnPriority::Protected
        } else if self.yield_groups.contains(&g) {
            TurnPriority::Yield
        } else {
            TurnPriority::Banned
        }
    }

    pub fn edit_group(&mut self, g: &TurnGroup, pri: TurnPriority) {
        let mut ids = vec![g.id];
        if g.turn_type == TurnType::Crosswalk {
            ids.push(TurnGroupID {
                from: g.id.to,
                to: g.id.from,
                parent: g.id.parent,
                crosswalk: true,
            });
        }
        for id in ids {
            self.protected_groups.remove(&id);
            self.yield_groups.remove(&id);
            if pri == TurnPriority::Protected {
                self.protected_groups.insert(id);
            } else if pri == TurnPriority::Yield {
                self.yield_groups.insert(id);
            }
        }
    }
}

// Add all possible protected groups to existing phases.
fn expand_all_phases(phases: &mut Vec<Phase>, turn_groups: &BTreeMap<TurnGroupID, TurnGroup>) {
    for phase in phases.iter_mut() {
        for g in turn_groups.keys() {
            if phase.could_be_protected(*g, turn_groups) {
                phase.protected_groups.insert(*g);
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
    // TODO Could pass this in instead of recompute...
    let turn_groups = TurnGroup::for_i(i, map);
    let mut phases: Vec<Phase> = Vec::new();

    for specs in phase_specs {
        let mut phase = Phase::new();

        for (roads, turn_type, protected) in specs.into_iter() {
            for group in turn_groups.values() {
                if !roads.contains(&group.id.from.id) || turn_type != group.turn_type {
                    continue;
                }

                phase.edit_group(
                    group,
                    if protected {
                        TurnPriority::Protected
                    } else {
                        TurnPriority::Yield
                    },
                );
            }
        }

        // Filter out empty phases if they happen.
        if phase.protected_groups.is_empty() && phase.yield_groups.is_empty() {
            continue;
        }

        phases.push(phase);
    }

    phases
}

impl ControlTrafficSignal {
    pub fn export(&self, map: &Map) -> seattle_traffic_signals::TrafficSignal {
        seattle_traffic_signals::TrafficSignal {
            intersection_osm_node_id: map.get_i(self.id).orig_id.osm_node_id,
            phases: self
                .phases
                .iter()
                .map(|p| seattle_traffic_signals::Phase {
                    protected_turns: p
                        .protected_groups
                        .iter()
                        .map(|t| export_turn_group(t, map))
                        .collect(),
                    permitted_turns: p
                        .yield_groups
                        .iter()
                        .map(|t| export_turn_group(t, map))
                        .collect(),
                    duration_seconds: p.duration.inner_seconds() as usize,
                })
                .collect(),
        }
    }

    pub fn import(
        raw: seattle_traffic_signals::TrafficSignal,
        id: IntersectionID,
        map: &Map,
    ) -> Option<ControlTrafficSignal> {
        let mut phases = Vec::new();
        for p in raw.phases {
            let num_protected = p.protected_turns.len();
            let num_permitted = p.permitted_turns.len();
            let protected_groups = p
                .protected_turns
                .into_iter()
                .filter_map(|t| import_turn_group(t, map))
                .collect::<BTreeSet<_>>();
            let yield_groups = p
                .permitted_turns
                .into_iter()
                .filter_map(|t| import_turn_group(t, map))
                .collect::<BTreeSet<_>>();
            if protected_groups.len() == num_protected && yield_groups.len() == num_permitted {
                phases.push(Phase {
                    protected_groups,
                    yield_groups,
                    duration: Duration::seconds(p.duration_seconds as f64),
                });
            } else {
                return None;
            }
        }
        ControlTrafficSignal {
            id,
            phases,
            offset: Duration::ZERO,
            turn_groups: TurnGroup::for_i(id, map),
        }
        .validate()
        .ok()
    }
}

fn export_turn_group(id: &TurnGroupID, map: &Map) -> seattle_traffic_signals::Turn {
    let from = map.get_r(id.from.id).orig_id;
    let to = map.get_r(id.to.id).orig_id;

    seattle_traffic_signals::Turn {
        from: seattle_traffic_signals::DirectedRoad {
            osm_way_id: from.osm_way_id,
            osm_node1: from.i1.osm_node_id,
            osm_node2: from.i2.osm_node_id,
            is_forwards: id.from.forwards,
        },
        to: seattle_traffic_signals::DirectedRoad {
            osm_way_id: to.osm_way_id,
            osm_node1: to.i1.osm_node_id,
            osm_node2: to.i2.osm_node_id,
            is_forwards: id.to.forwards,
        },
        intersection_osm_node_id: map.get_i(id.parent).orig_id.osm_node_id,
        is_crosswalk: id.crosswalk,
    }
}

fn import_turn_group(id: seattle_traffic_signals::Turn, map: &Map) -> Option<TurnGroupID> {
    Some(TurnGroupID {
        from: find_r(id.from, map)?,
        to: find_r(id.to, map)?,
        parent: map.find_i_by_osm_id(id.intersection_osm_node_id).ok()?,
        crosswalk: id.is_crosswalk,
    })
}

fn find_r(id: seattle_traffic_signals::DirectedRoad, map: &Map) -> Option<DirectedRoadID> {
    Some(DirectedRoadID {
        id: map
            .find_r_by_osm_id(id.osm_way_id, (id.osm_node1, id.osm_node2))
            .ok()?,
        forwards: id.is_forwards,
    })
}

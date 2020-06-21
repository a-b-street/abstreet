use crate::{
    ControlTrafficSignal, IntersectionID, Map, Phase, RoadID, TurnGroup, TurnGroupID, TurnPriority,
    TurnType,
};
use abstutil::Timer;
use geom::Duration;
use std::collections::BTreeMap;

pub fn get_possible_policies(
    map: &Map,
    id: IntersectionID,
    timer: &mut Timer,
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
            timer.error(format!(
                "seattle_traffic_signals data for {} out of date, go update it",
                map.get_i(id).orig_id.osm_node_id
            ));
        }
    }

    // As long as we're using silly heuristics for these by default, prefer shorter cycle
    // length.
    if let Some(ts) = four_way_two_phase(map, id) {
        results.push(("two-phase".to_string(), ts));
    }
    if let Some(ts) = three_way(map, id) {
        results.push(("three-phase".to_string(), ts));
    }
    if let Some(ts) = four_way_four_phase(map, id) {
        results.push(("four-phase".to_string(), ts));
    }
    if let Some(ts) = degenerate(map, id) {
        results.push(("degenerate (2 roads)".to_string(), ts));
    }
    if let Some(ts) = four_oneways(map, id) {
        results.push(("two-phase for 4 one-ways".to_string(), ts));
    }
    if let Some(ts) = phase_per_road(map, id) {
        results.push(("phase per road".to_string(), ts));
    }
    results.push((
        "arbitrary assignment".to_string(),
        greedy_assignment(map, id),
    ));
    results.push((
        "all walk, then free-for-all yield".to_string(),
        all_walk_all_yield(map, id),
    ));
    results
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

pub fn brute_force(map: &Map, i: IntersectionID) {
    let turn_groups: Vec<TurnGroup> = TurnGroup::for_i(i, map)
        .into_iter()
        .filter_map(|(id, tg)| if id.crosswalk { None } else { Some(tg) })
        .collect();
    let indices: Vec<usize> = (0..turn_groups.len()).collect();
    for num_phases in 1..=turn_groups.len() {
        println!(
            "For {} turn groups, looking for solution with {} phases",
            turn_groups.len(),
            num_phases
        );
        for partition in helper(&indices, num_phases) {
            if okay_partition(turn_groups.iter().collect(), partition) {
                return;
            }
        }
    }
    unreachable!()
}

fn okay_partition(turn_groups: Vec<&TurnGroup>, partition: Partition) -> bool {
    for phase in partition.0 {
        let mut protected: Vec<&TurnGroup> = Vec::new();
        for idx in phase {
            let tg = turn_groups[idx];
            if protected.iter().any(|other| tg.conflicts_with(other)) {
                return false;
            }
            protected.push(tg);
        }
    }
    println!("found one that works! :O");
    true
}

// Technically, a set of sets; order doesn't matter
#[derive(Clone)]
struct Partition(Vec<Vec<usize>>);

// Extremely hasty port of https://stackoverflow.com/a/30903689
fn helper(items: &[usize], max_size: usize) -> Vec<Partition> {
    if items.len() < max_size || max_size == 0 {
        return Vec::new();
    }

    if max_size == 1 {
        return vec![Partition(vec![items.to_vec()])];
    }

    let mut results = Vec::new();
    let prev1 = helper(&items[0..items.len() - 1], max_size);
    for i in 0..prev1.len() {
        for j in 0..prev1[i].0.len() {
            let mut partition: Vec<Vec<usize>> = Vec::new();
            for inner in &prev1[i].0 {
                partition.push(inner.clone());
            }
            partition[j].push(*items.last().unwrap());
            results.push(Partition(partition));
        }
    }

    let set = vec![*items.last().unwrap()];
    for mut partition in helper(&items[0..items.len() - 1], max_size - 1) {
        partition.0.push(set.clone());
        results.push(partition);
    }
    results
}

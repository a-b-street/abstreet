use crate::{
    ControlTrafficSignal, IntersectionCluster, IntersectionID, Map, Phase, PhaseType, RoadID,
    TurnGroup, TurnGroupID, TurnPriority, TurnType,
};
use abstutil::Timer;
use geom::Duration;
use std::collections::HashSet;

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
        .remove(&map.get_i(id).orig_id.0)
    {
        if let Ok(ts) = ControlTrafficSignal::import(raw, id, map) {
            results.push(("hand-mapped current real settings".to_string(), ts));
        } else {
            let i = map.get_i(id);
            timer.error(format!(
                "seattle_traffic_signals data for {} ({}) out of date, go update it",
                i.orig_id,
                i.name(map)
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
    if let Some(ts) = half_signal(map, id) {
        results.push(("half signal (2 roads with crosswalk)".to_string(), ts));
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

fn new(id: IntersectionID, map: &Map) -> ControlTrafficSignal {
    ControlTrafficSignal {
        id,
        phases: Vec::new(),
        offset: Duration::ZERO,
        turn_groups: TurnGroup::for_i(id, map).unwrap(),
    }
}

fn greedy_assignment(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    let mut ts = new(i, map);

    // Greedily partition groups into phases that only have protected groups.
    let mut remaining_groups: Vec<TurnGroupID> = ts.turn_groups.keys().cloned().collect();
    let mut current_phase = Phase::new();
    loop {
        let add = remaining_groups
            .iter()
            .position(|&g| current_phase.could_be_protected(g, &ts.turn_groups));
        match add {
            Some(idx) => {
                current_phase
                    .protected_groups
                    .insert(remaining_groups.remove(idx));
            }
            None => {
                assert!(!current_phase.protected_groups.is_empty());
                ts.phases.push(current_phase);
                current_phase = Phase::new();
                if remaining_groups.is_empty() {
                    break;
                }
            }
        }
    }

    expand_all_phases(&mut ts);

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

    let mut ts = new(i, map);
    make_phases(
        &mut ts,
        vec![vec![(vec![r1, r2], TurnType::Straight, PROTECTED)]],
    );
    ts.validate().ok()
}

fn half_signal(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    if map.get_i(i).roads.len() != 2 {
        return None;
    }

    let mut ts = new(i, map);
    let mut vehicle_phase = Phase::new();
    let mut ped_phase = Phase::new();
    for (id, group) in &ts.turn_groups {
        if id.crosswalk {
            ped_phase.edit_group(group, TurnPriority::Protected);
        } else {
            vehicle_phase.edit_group(group, TurnPriority::Protected);
        }
    }
    vehicle_phase.phase_type = PhaseType::Fixed(Duration::minutes(1));
    ped_phase.phase_type = PhaseType::Fixed(Duration::seconds(10.0));

    ts.phases = vec![vehicle_phase, ped_phase];
    ts.validate().ok()
}

fn three_way(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    if map.get_i(i).roads.len() != 3 {
        return None;
    }
    let mut ts = new(i, map);

    // Picture a T intersection. Use turn angles to figure out the "main" two roads.
    let straight = ts
        .turn_groups
        .values()
        .find(|g| g.turn_type == TurnType::Straight)?;
    let (north, south) = (straight.id.from.id, straight.id.to.id);
    let mut roads = map.get_i(i).roads.clone();
    roads.remove(&north);
    roads.remove(&south);
    let east = roads.into_iter().next().unwrap();

    // Two-phase with no protected lefts, right turn on red, turning cars yield to peds
    make_phases(
        &mut ts,
        vec![
            vec![
                (vec![north, south], TurnType::Straight, PROTECTED),
                (vec![north, south], TurnType::Right, YIELD),
                (vec![north, south], TurnType::Left, YIELD),
                (vec![east], TurnType::Right, YIELD),
            ],
            vec![
                (vec![east], TurnType::Straight, PROTECTED),
                (vec![east], TurnType::Right, YIELD),
                (vec![east], TurnType::Left, YIELD),
                (vec![north, south], TurnType::Right, YIELD),
            ],
        ],
    );

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
    let mut ts = new(i, map);
    make_phases(
        &mut ts,
        vec![
            vec![
                (vec![north, south], TurnType::Straight, PROTECTED),
                (vec![north, south], TurnType::Right, YIELD),
                (vec![east, west], TurnType::Right, YIELD),
            ],
            vec![(vec![north, south], TurnType::Left, PROTECTED)],
            vec![
                (vec![east, west], TurnType::Straight, PROTECTED),
                (vec![east, west], TurnType::Right, YIELD),
                (vec![north, south], TurnType::Right, YIELD),
            ],
            vec![(vec![east, west], TurnType::Left, PROTECTED)],
        ],
    );
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
    let mut ts = new(i, map);
    make_phases(
        &mut ts,
        vec![
            vec![
                (vec![north, south], TurnType::Straight, PROTECTED),
                (vec![north, south], TurnType::Right, YIELD),
                (vec![north, south], TurnType::Left, YIELD),
                (vec![east, west], TurnType::Right, YIELD),
            ],
            vec![
                (vec![east, west], TurnType::Straight, PROTECTED),
                (vec![east, west], TurnType::Right, YIELD),
                (vec![east, west], TurnType::Left, YIELD),
                (vec![north, south], TurnType::Right, YIELD),
            ],
        ],
    );
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
    let mut ts = new(i, map);
    make_phases(
        &mut ts,
        vec![
            vec![
                (vec![r1], TurnType::Straight, PROTECTED),
                // TODO Technically, upgrade to protected if there's no opposing crosswalk --
                // even though it doesn't matter much.
                (vec![r1], TurnType::Right, YIELD),
                (vec![r1], TurnType::Left, YIELD),
                (vec![r1], TurnType::Right, YIELD),
                // TODO Refactor
            ],
            vec![
                (vec![r2], TurnType::Straight, PROTECTED),
                // TODO Technically, upgrade to protected if there's no opposing crosswalk --
                // even though it doesn't matter much.
                (vec![r2], TurnType::Right, YIELD),
                (vec![r2], TurnType::Left, YIELD),
                (vec![r2], TurnType::Right, YIELD),
            ],
        ],
    );
    ts.validate().ok()
}

fn all_walk_all_yield(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    let mut ts = new(i, map);

    let mut all_walk = Phase::new();
    let mut all_yield = Phase::new();

    for group in ts.turn_groups.values() {
        match group.turn_type {
            TurnType::Crosswalk => {
                all_walk.protected_groups.insert(group.id);
            }
            _ => {
                all_yield.yield_groups.insert(group.id);
            }
        }
    }

    ts.phases = vec![all_walk, all_yield];
    // This must succeed
    ts.validate().unwrap()
}

fn phase_per_road(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    let mut ts = new(i, map);

    let sorted_roads = map
        .get_i(i)
        .get_roads_sorted_by_incoming_angle(map.all_roads());
    for idx in 0..sorted_roads.len() {
        let r = sorted_roads[idx];
        let adj1 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) - 1);
        let adj2 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) + 1);

        let mut phase = Phase::new();
        for group in ts.turn_groups.values() {
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
            ts.phases.push(phase);
        }
    }
    ts.validate().ok()
}

// Add all possible protected groups to existing phases.
fn expand_all_phases(ts: &mut ControlTrafficSignal) {
    for phase in ts.phases.iter_mut() {
        for g in ts.turn_groups.keys() {
            if phase.could_be_protected(*g, &ts.turn_groups) {
                phase.protected_groups.insert(*g);
            }
        }
    }
}

const PROTECTED: bool = true;
const YIELD: bool = false;

fn make_phases(
    ts: &mut ControlTrafficSignal,
    phase_specs: Vec<Vec<(Vec<RoadID>, TurnType, bool)>>,
) {
    for specs in phase_specs {
        let mut phase = Phase::new();

        for (roads, turn_type, protected) in specs.into_iter() {
            for group in ts.turn_groups.values() {
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

        // Add in all compatible crosswalks. Specifying this in specs explicitly doesn't work when
        // crosswalks stretch across a road strangely, which happens when one side of a road is
        // missing a sidewalk.
        // TODO If a phase has no protected turns at all, this adds the crosswalk to multiple
        // phases in a pretty weird way. It'd be better to add to just one phase -- the one with
        // the least conflicting yields.
        for group in ts.turn_groups.values() {
            if group.turn_type == TurnType::Crosswalk
                && phase.could_be_protected(group.id, &ts.turn_groups)
            {
                phase.edit_group(group, TurnPriority::Protected);
            }
        }

        // Filter out empty phases if they happen.
        if phase.protected_groups.is_empty() && phase.yield_groups.is_empty() {
            continue;
        }

        ts.phases.push(phase);
    }

    if ts.phases.len() > 1 {
        // At intersections of one-ways like Terry and Denny, we could get away with a single phase.
        // Really weak form of this now, just collapsing the one smallest phase.
        let smallest = ts
            .phases
            .iter()
            .min_by_key(|p| p.protected_groups.len() + p.yield_groups.len())
            .cloned()
            .unwrap();
        if ts.phases.iter().any(|p| {
            p != &smallest
                && smallest.protected_groups.is_subset(&p.protected_groups)
                && smallest.yield_groups.is_subset(&p.yield_groups)
        }) {
            ts.phases.retain(|p| p != &smallest);
        }
    }
}

pub fn brute_force(map: &Map, i: IntersectionID) {
    let turn_groups: Vec<TurnGroup> = TurnGroup::for_i(i, map)
        .unwrap()
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

pub fn synchronize(map: &mut Map) {
    let mut seen = HashSet::new();
    let mut pairs = Vec::new();
    let handmapped = seattle_traffic_signals::load_all_data().unwrap();
    for i in map.all_intersections() {
        if !i.is_traffic_signal() || seen.contains(&i.id) || handmapped.contains_key(&i.orig_id.0) {
            continue;
        }
        if let Some(list) = IntersectionCluster::autodetect(i.id, map) {
            let list = list.into_iter().collect::<Vec<_>>();
            if list.len() == 2
                && map.get_traffic_signal(list[0]).phases.len() == 2
                && map.get_traffic_signal(list[1]).phases.len() == 2
            {
                pairs.push((list[0], list[1]));
                seen.insert(list[0]);
                seen.insert(list[1]);
            }
        }
    }

    for (i1, i2) in pairs {
        let ts1 = map.get_traffic_signal(i1);
        let ts2 = map.get_traffic_signal(i2);
        let flip1 = ts1.phases[0].protected_groups.iter().any(|tg1| {
            !tg1.crosswalk
                && ts2.phases[1]
                    .protected_groups
                    .iter()
                    .any(|tg2| !tg2.crosswalk && (tg1.to == tg2.from || tg1.from == tg2.to))
        });
        let flip2 = ts1.phases[1].protected_groups.iter().any(|tg1| {
            !tg1.crosswalk
                && ts2.phases[0]
                    .protected_groups
                    .iter()
                    .any(|tg2| !tg2.crosswalk && (tg1.to == tg2.from || tg1.from == tg2.to))
        });
        if flip1 || flip2 {
            println!(
                "Flipping phase order of {} and {} to synchronize them",
                i1, i2
            );
            map.traffic_signals.get_mut(&i1).unwrap().phases.swap(0, 1);
        }
    }
}

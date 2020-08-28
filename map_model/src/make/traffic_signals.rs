use crate::{
    ControlTrafficSignal, IntersectionCluster, IntersectionID, Map, PhaseType, RoadID, Stage,
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
                i.name(None, map)
            ));
        }
    }

    // As long as we're using silly heuristics for these by default, prefer shorter cycle
    // length.
    if let Some(ts) = four_way_two_stage(map, id) {
        results.push(("two-stage".to_string(), ts));
    }
    if let Some(ts) = three_way(map, id) {
        results.push(("three-stage".to_string(), ts));
    }
    if let Some(ts) = four_way_four_stage(map, id) {
        results.push(("four-stage".to_string(), ts));
    }
    if let Some(ts) = half_signal(map, id) {
        results.push(("half signal (2 roads with crosswalk)".to_string(), ts));
    }
    if let Some(ts) = degenerate(map, id) {
        results.push(("degenerate (2 roads)".to_string(), ts));
    }
    if let Some(ts) = four_oneways(map, id) {
        results.push(("two-stage for 4 one-ways".to_string(), ts));
    }
    if let Some(ts) = stage_per_road(map, id) {
        results.push(("stage per road".to_string(), ts));
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
        stages: Vec::new(),
        offset: Duration::ZERO,
        turn_groups: TurnGroup::for_i(id, map).unwrap(),
    }
}

fn greedy_assignment(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    let mut ts = new(i, map);

    // Greedily partition groups into stages that only have protected groups.
    let mut remaining_groups: Vec<TurnGroupID> = ts.turn_groups.keys().cloned().collect();
    let mut current_stage = Stage::new();
    loop {
        let add = remaining_groups
            .iter()
            .position(|&g| current_stage.could_be_protected(g, &ts.turn_groups));
        match add {
            Some(idx) => {
                current_stage
                    .protected_groups
                    .insert(remaining_groups.remove(idx));
            }
            None => {
                assert!(!current_stage.protected_groups.is_empty());
                ts.stages.push(current_stage);
                current_stage = Stage::new();
                if remaining_groups.is_empty() {
                    break;
                }
            }
        }
    }

    expand_all_stages(&mut ts);

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
    make_stages(
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
    let mut vehicle_stage = Stage::new();
    let mut ped_stage = Stage::new();
    for (id, group) in &ts.turn_groups {
        if id.crosswalk {
            ped_stage.edit_group(group, TurnPriority::Protected);
        } else {
            vehicle_stage.edit_group(group, TurnPriority::Protected);
        }
    }
    vehicle_stage.phase_type = PhaseType::Fixed(Duration::minutes(1));
    ped_stage.phase_type = PhaseType::Fixed(Duration::seconds(10.0));

    ts.stages = vec![vehicle_stage, ped_stage];
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

    // Two-stage with no protected lefts, right turn on red, turning cars yield to peds
    make_stages(
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

fn four_way_four_stage(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    if map.get_i(i).roads.len() != 4 {
        return None;
    }

    // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
    let roads = map
        .get_i(i)
        .get_roads_sorted_by_incoming_angle(map.all_roads());
    let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

    // Four-stage with protected lefts, right turn on red (except for the protected lefts),
    // turning cars yield to peds
    let mut ts = new(i, map);
    make_stages(
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

fn four_way_two_stage(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    if map.get_i(i).roads.len() != 4 {
        return None;
    }

    // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
    let roads = map
        .get_i(i)
        .get_roads_sorted_by_incoming_angle(map.all_roads());
    let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

    // Two-stage with no protected lefts, right turn on red, turning cars yielding to peds
    let mut ts = new(i, map);
    make_stages(
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
    make_stages(
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

    let mut all_walk = Stage::new();
    let mut all_yield = Stage::new();

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

    ts.stages = vec![all_walk, all_yield];
    // This must succeed
    ts.validate().unwrap()
}

fn stage_per_road(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    let mut ts = new(i, map);

    let sorted_roads = map
        .get_i(i)
        .get_roads_sorted_by_incoming_angle(map.all_roads());
    for idx in 0..sorted_roads.len() {
        let r = sorted_roads[idx];
        let adj1 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) - 1);
        let adj2 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) + 1);

        let mut stage = Stage::new();
        for group in ts.turn_groups.values() {
            if group.turn_type == TurnType::Crosswalk {
                if group.id.from.id == adj1 || group.id.from.id == adj2 {
                    stage.protected_groups.insert(group.id);
                }
            } else if group.id.from.id == r {
                stage.yield_groups.insert(group.id);
            }
        }
        // Might have a one-way outgoing road. Skip it.
        if !stage.yield_groups.is_empty() {
            ts.stages.push(stage);
        }
    }
    ts.validate().ok()
}

// Add all possible protected groups to existing stages.
fn expand_all_stages(ts: &mut ControlTrafficSignal) {
    for stage in ts.stages.iter_mut() {
        for g in ts.turn_groups.keys() {
            if stage.could_be_protected(*g, &ts.turn_groups) {
                stage.protected_groups.insert(*g);
            }
        }
    }
}

const PROTECTED: bool = true;
const YIELD: bool = false;

fn make_stages(
    ts: &mut ControlTrafficSignal,
    stage_specs: Vec<Vec<(Vec<RoadID>, TurnType, bool)>>,
) {
    for specs in stage_specs {
        let mut stage = Stage::new();

        for (roads, turn_type, protected) in specs.into_iter() {
            for group in ts.turn_groups.values() {
                if !roads.contains(&group.id.from.id) || turn_type != group.turn_type {
                    continue;
                }

                stage.edit_group(
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
        // TODO If a stage has no protected turns at all, this adds the crosswalk to multiple
        // stages in a pretty weird way. It'd be better to add to just one stage -- the one with
        // the least conflicting yields.
        for group in ts.turn_groups.values() {
            if group.turn_type == TurnType::Crosswalk
                && stage.could_be_protected(group.id, &ts.turn_groups)
            {
                stage.edit_group(group, TurnPriority::Protected);
            }
        }

        // Filter out empty stages if they happen.
        if stage.protected_groups.is_empty() && stage.yield_groups.is_empty() {
            continue;
        }

        ts.stages.push(stage);
    }

    if ts.stages.len() > 1 {
        // At intersections of one-ways like Terry and Denny, we could get away with a single stage.
        // Really weak form of this now, just collapsing the one smallest stage.
        let smallest = ts
            .stages
            .iter()
            .min_by_key(|p| p.protected_groups.len() + p.yield_groups.len())
            .cloned()
            .unwrap();
        if ts.stages.iter().any(|p| {
            p != &smallest
                && smallest.protected_groups.is_subset(&p.protected_groups)
                && smallest.yield_groups.is_subset(&p.yield_groups)
        }) {
            ts.stages.retain(|p| p != &smallest);
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
    for num_stages in 1..=turn_groups.len() {
        println!(
            "For {} turn groups, looking for solution with {} stages",
            turn_groups.len(),
            num_stages
        );
        for partition in helper(&indices, num_stages) {
            if okay_partition(turn_groups.iter().collect(), partition) {
                return;
            }
        }
    }
    unreachable!()
}

fn okay_partition(turn_groups: Vec<&TurnGroup>, partition: Partition) -> bool {
    for stage in partition.0 {
        let mut protected: Vec<&TurnGroup> = Vec::new();
        for idx in stage {
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
                && map.get_traffic_signal(list[0]).stages.len() == 2
                && map.get_traffic_signal(list[1]).stages.len() == 2
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
        let flip1 = ts1.stages[0].protected_groups.iter().any(|tg1| {
            !tg1.crosswalk
                && ts2.stages[1]
                    .protected_groups
                    .iter()
                    .any(|tg2| !tg2.crosswalk && (tg1.to == tg2.from || tg1.from == tg2.to))
        });
        let flip2 = ts1.stages[1].protected_groups.iter().any(|tg1| {
            !tg1.crosswalk
                && ts2.stages[0]
                    .protected_groups
                    .iter()
                    .any(|tg2| !tg2.crosswalk && (tg1.to == tg2.from || tg1.from == tg2.to))
        });
        if flip1 || flip2 {
            println!(
                "Flipping stage order of {} and {} to synchronize them",
                i1, i2
            );
            map.traffic_signals.get_mut(&i1).unwrap().stages.swap(0, 1);
        }
    }
}

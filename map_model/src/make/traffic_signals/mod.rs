//! The various traffic signal generators live in the traffic signal module. Eventually, we
//! might want to move to a trait. For now, there's a single make_traffic_signal static method
//! in each generator file, which is called to generate a traffic signal of a particular flavor.
//!
//! For example, lagging_green.rs contains a one public fn:
//!     pub fn make_traffic_signal(map: &Map, i: IntersectionID)->Option<ControlTrafficSignal>

use std::collections::{BTreeSet, HashSet};

use crate::{
    ControlTrafficSignal, DrivingSide, IntersectionCluster, IntersectionID, Map, Movement,
    MovementID, RoadID, Stage, StageType, TurnPriority, TurnType,
};
use geom::Duration;

mod lagging_green;

/// Applies a bunch of heuristics to a single intersection, returning the valid results in
/// best-first order. The signal configuration is only based on the roads connected to the
/// intersection.
///
/// If `enforce_manual_signals` is true, then any data from the `traffic_signal_data` crate that
/// matches the map will be validated against the current map. If the config is out-of-date, this
/// method will panic, so that whoever is running the importer can immediately fix the config.
pub fn get_possible_policies(
    map: &Map,
    id: IntersectionID,
    enforce_manual_signals: bool,
) -> Vec<(String, ControlTrafficSignal)> {
    let mut results = Vec::new();

    if let Some(raw) = traffic_signal_data::load_all_data()
        .unwrap()
        .remove(&map.get_i(id).orig_id.0)
    {
        match ControlTrafficSignal::import(raw, id, map).and_then(|ts| ts.validate().map(|_| ts)) {
            Ok(ts) => {
                results.push(("manually specified settings".to_string(), ts));
            }
            Err(err) => {
                let i = map.get_i(id);
                if enforce_manual_signals {
                    panic!(
                        "traffic_signal_data data for {} ({}) out of date, go update it: {}",
                        i.orig_id,
                        i.name(None, map),
                        err
                    );
                } else {
                    warn!(
                        "traffic_signal_data data for {} no longer valid with map edits: {}",
                        i.orig_id, err
                    );
                }
            }
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
    if let Some(ts) = lagging_green::make_traffic_signal(map, id) {
        results.push(("lagging green".to_string(), ts));
    }
    results.push(("stage per road".to_string(), stage_per_road(map, id)));
    results.push((
        "arbitrary assignment".to_string(),
        greedy_assignment(map, id),
    ));
    results.push((
        "all walk, then free-for-all yield".to_string(),
        all_walk_all_yield(map, id),
    ));

    // Make sure all possible policies have a minimum crosswalk time enforced
    for (_, signal) in &mut results {
        for stage in &mut signal.stages {
            let crosswalks: Vec<MovementID> = stage
                .protected_movements
                .iter()
                .filter(|id| id.crosswalk)
                .cloned()
                .collect();
            for id in crosswalks {
                stage.enforce_minimum_crosswalk_time(&signal.movements[&id]);
            }
        }
    }

    results.retain(|pair| pair.1.validate().is_ok());
    results
}

fn new(id: IntersectionID, map: &Map) -> ControlTrafficSignal {
    ControlTrafficSignal {
        id,
        stages: Vec::new(),
        offset: Duration::ZERO,
        movements: Movement::for_i(id, map).unwrap(),
    }
}

fn greedy_assignment(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    let mut ts = new(i, map);

    // Greedily partition movements into stages that only have protected movements.
    let mut remaining_movements: Vec<MovementID> = ts.movements.keys().cloned().collect();
    let mut current_stage = Stage::new();
    loop {
        let add = remaining_movements
            .iter()
            .position(|&g| current_stage.could_be_protected(g, &ts.movements));
        match add {
            Some(idx) => {
                current_stage
                    .protected_movements
                    .insert(remaining_movements.remove(idx));
            }
            None => {
                assert!(!current_stage.protected_movements.is_empty());
                ts.stages.push(current_stage);
                current_stage = Stage::new();
                if remaining_movements.is_empty() {
                    break;
                }
            }
        }
    }

    expand_all_stages(&mut ts);

    ts
}

fn degenerate(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    let roads = get_sorted_incoming_roads(i, map);
    if roads.len() != 2 {
        return None;
    }
    let (r1, r2) = (roads[0], roads[1]);

    let mut ts = new(i, map);
    make_stages(
        &mut ts,
        map.config.driving_side,
        vec![vec![(vec![r1, r2], TurnType::Straight, PROTECTED)]],
    );
    Some(ts)
}

fn half_signal(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    if map.get_i(i).roads.len() != 2 {
        return None;
    }

    let mut ts = new(i, map);
    let mut vehicle_stage = Stage::new();
    let mut ped_stage = Stage::new();
    for (id, movement) in &ts.movements {
        if id.crosswalk {
            ped_stage.edit_movement(movement, TurnPriority::Protected);
        } else {
            vehicle_stage.edit_movement(movement, TurnPriority::Protected);
        }
    }
    vehicle_stage.stage_type = StageType::Fixed(Duration::minutes(1));
    ped_stage.stage_type = StageType::Fixed(Duration::seconds(10.0));

    ts.stages = vec![vehicle_stage, ped_stage];
    Some(ts)
}

fn three_way(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    let roads = get_sorted_incoming_roads(i, map);
    if roads.len() != 3 {
        return None;
    }
    let mut ts = new(i, map);

    // Picture a T intersection. Use turn angles to figure out the "main" two roads.
    let straight = ts
        .movements
        .values()
        .find(|g| g.turn_type == TurnType::Straight)?;
    let (north, south) = (straight.id.from.id, straight.id.to.id);
    let east = roads
        .into_iter()
        .find(|r| *r != north && *r != south)
        .unwrap();

    // Two-stage with no protected lefts, right turn on red, turning cars yield to peds
    make_stages(
        &mut ts,
        map.config.driving_side,
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

    Some(ts)
}

fn four_way_four_stage(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    let roads = get_sorted_incoming_roads(i, map);
    if roads.len() != 4 {
        return None;
    }

    // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
    let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

    // Four-stage with protected lefts, right turn on red (except for the protected lefts),
    // turning cars yield to peds
    let mut ts = new(i, map);
    make_stages(
        &mut ts,
        map.config.driving_side,
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
    Some(ts)
}

fn four_way_two_stage(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
    let roads = get_sorted_incoming_roads(i, map);
    if roads.len() != 4 {
        return None;
    }

    // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
    let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

    // Two-stage with no protected lefts, right turn on red, turning cars yielding to peds
    let mut ts = new(i, map);
    make_stages(
        &mut ts,
        map.config.driving_side,
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
    Some(ts)
}

fn all_walk_all_yield(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    let mut ts = new(i, map);

    let mut all_walk = Stage::new();
    let mut all_yield = Stage::new();

    for movement in ts.movements.values() {
        match movement.turn_type {
            TurnType::Crosswalk => {
                all_walk.protected_movements.insert(movement.id);
            }
            _ => {
                all_yield.yield_movements.insert(movement.id);
            }
        }
    }

    ts.stages = vec![all_walk, all_yield];
    ts
}

fn stage_per_road(map: &Map, i: IntersectionID) -> ControlTrafficSignal {
    let mut ts = new(i, map);

    let sorted_roads = map
        .get_i(i)
        .get_roads_sorted_by_incoming_angle(map.all_roads());
    for idx in 0..sorted_roads.len() {
        let r = sorted_roads[idx];
        let adj1 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) - 1);
        let adj2 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) + 1);

        let mut stage = Stage::new();
        for movement in ts.movements.values() {
            if movement.turn_type == TurnType::Crosswalk {
                if movement.id.from.id == adj1 || movement.id.from.id == adj2 {
                    stage.protected_movements.insert(movement.id);
                }
            } else if movement.id.from.id == r {
                stage.yield_movements.insert(movement.id);
            }
        }
        // Might have a one-way outgoing road. Skip it.
        if !stage.yield_movements.is_empty() {
            ts.stages.push(stage);
        }
    }
    ts
}

// Add all possible protected movements to existing stages.
fn expand_all_stages(ts: &mut ControlTrafficSignal) {
    for stage in ts.stages.iter_mut() {
        for g in ts.movements.keys() {
            if stage.could_be_protected(*g, &ts.movements) {
                stage.protected_movements.insert(*g);
            }
        }
    }
}

const PROTECTED: bool = true;
const YIELD: bool = false;

fn make_stages(
    ts: &mut ControlTrafficSignal,
    driving_side: DrivingSide,
    stage_specs: Vec<Vec<(Vec<RoadID>, TurnType, bool)>>,
) {
    // Don't filter unprotected only stages
    make_stages_filtered(ts, driving_side, stage_specs, false);
}
fn make_stages_filtered(
    ts: &mut ControlTrafficSignal,
    driving_side: DrivingSide,
    stage_specs: Vec<Vec<(Vec<RoadID>, TurnType, bool)>>,
    filter_unprotected: bool,
) {
    for specs in stage_specs {
        let mut stage = Stage::new();

        for (roads, mut turn_type, protected) in specs.into_iter() {
            // The heuristics are written assuming right turns are easy and lefts are hard, so
            // invert in the UK.
            if driving_side == DrivingSide::Left {
                if turn_type == TurnType::Right {
                    turn_type = TurnType::Left;
                } else if turn_type == TurnType::Left {
                    turn_type = TurnType::Right;
                }
            }

            for movement in ts.movements.values() {
                if !roads.contains(&movement.id.from.id) || turn_type != movement.turn_type {
                    continue;
                }

                stage.edit_movement(
                    movement,
                    if protected {
                        TurnPriority::Protected
                    } else {
                        TurnPriority::Yield
                    },
                );
            }
        }
        // Filter out unprotected only stages if they happen.
        if filter_unprotected && stage.protected_movements.is_empty() {
            continue;
        }

        // Add in all compatible crosswalks. Specifying this in specs explicitly doesn't work when
        // crosswalks stretch across a road strangely, which happens when one side of a road is
        // missing a sidewalk.
        // TODO If a stage has no protected turns at all, this adds the crosswalk to multiple
        // stages in a pretty weird way. It'd be better to add to just one stage -- the one with
        // the least conflicting yields.
        for movement in ts.movements.values() {
            if movement.turn_type == TurnType::Crosswalk
                && stage.could_be_protected(movement.id, &ts.movements)
            {
                stage.edit_movement(movement, TurnPriority::Protected);
            }
        }

        // Filter out empty stages if they happen.
        if stage.protected_movements.is_empty() && stage.yield_movements.is_empty() {
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
            .min_by_key(|p| p.protected_movements.len() + p.yield_movements.len())
            .cloned()
            .unwrap();
        if ts.stages.iter().any(|p| {
            p != &smallest
                && smallest
                    .protected_movements
                    .is_subset(&p.protected_movements)
                && smallest.yield_movements.is_subset(&p.yield_movements)
        }) {
            ts.stages.retain(|p| p != &smallest);
        }
    }
}

/// Simple second-pass after generating all signals. Find pairs of traffic signals very close to
/// each other with 2 stages each, see if the primary movement of the first stages lead to each
/// other, and flip the order of stages if not. This is often wrong when the most common movement is
/// actually turning left then going straight (near Mercer for example), but not sure how we could
/// know that without demand data.
pub fn synchronize(map: &mut Map) {
    let mut seen = HashSet::new();
    let mut pairs = Vec::new();
    let handmapped = traffic_signal_data::load_all_data().unwrap();
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
        let flip1 = ts1.stages[0].protected_movements.iter().any(|m1| {
            !m1.crosswalk
                && ts2.stages[1]
                    .protected_movements
                    .iter()
                    .any(|m2| !m2.crosswalk && (m1.to == m2.from || m1.from == m2.to))
        });
        let flip2 = ts1.stages[1].protected_movements.iter().any(|m1| {
            !m1.crosswalk
                && ts2.stages[0]
                    .protected_movements
                    .iter()
                    .any(|m2| !m2.crosswalk && (m1.to == m2.from || m1.from == m2.to))
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

/// Return all incoming roads to an intersection, sorted by angle. This skips one-way roads
/// outbound from the intersection, since no turns originate from those anyway. This allows
/// heuristics for a 3-way intersection to not care if one of the roads happens to be a dual
/// carriageway (split into two one-ways).
fn get_sorted_incoming_roads(i: IntersectionID, map: &Map) -> Vec<RoadID> {
    let mut roads = Vec::new();
    for r in map
        .get_i(i)
        .get_roads_sorted_by_incoming_angle(map.all_roads())
    {
        if !map.get_r(r).incoming_lanes(i).is_empty() {
            roads.push(r);
        }
    }
    roads
}

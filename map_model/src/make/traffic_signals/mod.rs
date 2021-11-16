//! The various traffic signal generators live in the traffic signal module. Eventually, we
//! might want to move to a trait. For now, there's a single make_traffic_signal static method
//! in each generator file, which is called to generate a traffic signal of a particular flavor.
//!
//! For example, lagging_green.rs contains a one public fn:
//!     pub fn make_traffic_signal(map: &Map, i: IntersectionID)->Option<ControlTrafficSignal>

use std::collections::{BTreeSet, HashSet};

use crate::{
    ControlTrafficSignal, DrivingSide, Intersection, IntersectionCluster, IntersectionID, Map,
    MovementID, RoadID, Stage, StageType, TurnPriority, TurnType, MapConfig
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

    let i = map.get_i(id);
    if let Some(raw) = traffic_signal_data::load_all_data()
        .unwrap()
        .remove(&i.orig_id.0)
    {
        match ControlTrafficSignal::import(raw, id, map).and_then(|ts| ts.validate(i).map(|_| ts)) {
            Ok(ts) => {
                results.push(("manually specified settings".to_string(), ts));
            }
            Err(err) => {
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
    if let Some(ts) = four_way_two_stage(map, i) {
        results.push(("two-stage".to_string(), ts));
    }
    if let Some(ts) = three_way(map, i) {
        results.push(("three-stage".to_string(), ts));
    }
    if let Some(ts) = four_way_four_stage(map, i) {
        results.push(("four-stage".to_string(), ts));
    }
    if let Some(ts) = half_signal(i) {
        results.push(("half signal (2 roads with crosswalk)".to_string(), ts));
    }
    if let Some(ts) = degenerate(map, i) {
        results.push(("degenerate (2 roads)".to_string(), ts));
    }
    if let Some(ts) = lagging_green::make_traffic_signal(map, i) {
        results.push(("lagging green".to_string(), ts));
    }
    results.push(("stage per road".to_string(), stage_per_road(map, i)));
    results.push(("arbitrary assignment".to_string(), greedy_assignment(i)));
    results.push((
        "all walk, then free-for-all yield".to_string(),
        all_walk_all_yield(i),
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
                stage.enforce_minimum_crosswalk_time(&i.movements[&id]);
            }
        }
    }

    results.retain(|pair| pair.1.validate(i).is_ok());
    results
}

fn new(id: IntersectionID) -> ControlTrafficSignal {
    ControlTrafficSignal {
        id,
        stages: Vec::new(),
        offset: Duration::ZERO,
    }
}

fn greedy_assignment(i: &Intersection) -> ControlTrafficSignal {
    let mut ts = new(i.id);

    // Greedily partition movements into stages that only have protected movements.
    let mut remaining_movements: Vec<MovementID> = i.movements.keys().cloned().collect();
    let mut current_stage = Stage::new();
    loop {
        let add = remaining_movements
            .iter()
            .position(|&g| current_stage.could_be_protected(g, i));
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

    expand_all_stages(&mut ts, i);

    ts
}

fn degenerate(map: &Map, i: &Intersection) -> Option<ControlTrafficSignal> {
    let roads = i.get_sorted_incoming_roads(map);
    if roads.len() != 2 {
        return None;
    }
    let (r1, r2) = (roads[0], roads[1]);

    let mut ts = new(i.id);
    make_stages(
        &mut ts,
        &map.config,
        i,
        vec![vec![(vec![r1, r2], TurnType::Straight, PROTECTED)]],
    );
    Some(ts)
}

fn half_signal(i: &Intersection) -> Option<ControlTrafficSignal> {
    if i.roads.len() != 2 {
        return None;
    }

    let mut ts = new(i.id);
    let mut vehicle_stage = Stage::new();
    let mut ped_stage = Stage::new();
    for (id, movement) in &i.movements {
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

fn three_way(map: &Map, i: &Intersection) -> Option<ControlTrafficSignal> {
    let roads = i.get_sorted_incoming_roads(map);
    if roads.len() != 3 {
        return None;
    }
    let mut ts = new(i.id);

    // Picture a T intersection. Use turn angles to figure out the "main" two roads.
    let straight = i
        .movements
        .values()
        .find(|g| g.turn_type == TurnType::Straight)?;
    let (north, south) = (straight.id.from.road, straight.id.to.road);
    let east = roads
        .into_iter()
        .find(|r| *r != north && *r != south)
        .unwrap();

    // Two-stage with no protected lefts, right turn on red, turning cars yield to peds
    make_stages(
        &mut ts,
        &map.config,
        i,
        vec![
            vec![
                (vec![north, south], TurnType::Straight, PROTECTED),
                (vec![north, south], TurnType::Right, YIELD),
                (vec![north, south], TurnType::Left, YIELD),
                (vec![east], TurnType::Right, YIELD),
                (vec![east], TurnType::Crosswalk, PROTECTED),
                // TODO Maybe UnmarkedCrossing should yield
                (vec![east], TurnType::UnmarkedCrossing, PROTECTED),
            ],
            vec![
                (vec![east], TurnType::Straight, PROTECTED),
                (vec![east], TurnType::Right, YIELD),
                (vec![east], TurnType::Left, YIELD),
                (vec![north, south], TurnType::Right, YIELD),
                (vec![north, south], TurnType::Crosswalk, PROTECTED),
                (vec![north, south], TurnType::UnmarkedCrossing, PROTECTED),
            ],
        ],
    );

    Some(ts)
}

fn four_way_four_stage(map: &Map, i: &Intersection) -> Option<ControlTrafficSignal> {
    let roads = i.get_sorted_incoming_roads(map);
    if roads.len() != 4 {
        return None;
    }

    // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
    let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

    // Four-stage with protected lefts, right turn on red (except for the protected lefts),
    // turning cars yield to peds
    let mut ts = new(i.id);
    make_stages(
        &mut ts,
        &map.config,
        i,
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

fn four_way_two_stage(map: &Map, i: &Intersection) -> Option<ControlTrafficSignal> {
    let roads = i.get_sorted_incoming_roads(map);
    if roads.len() != 4 {
        return None;
    }

    // Just to refer to these easily, label with directions. Imagine an axis-aligned four-way.
    let (north, west, south, east) = (roads[0], roads[1], roads[2], roads[3]);

    // Two-stage with no protected lefts, right turn on red, turning cars yielding to peds
    let mut ts = new(i.id);
    make_stages(
        &mut ts,
        &map.config,
        i,
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

fn all_walk_all_yield(i: &Intersection) -> ControlTrafficSignal {
    let mut ts = new(i.id);

    let mut all_walk = Stage::new();
    let mut all_yield = Stage::new();

    for movement in i.movements.values() {
        if movement.turn_type.pedestrian_crossing() {
            all_walk.protected_movements.insert(movement.id);
        } else {
            all_yield.yield_movements.insert(movement.id);
        }
    }

    ts.stages = vec![all_walk, all_yield];
    ts
}

fn stage_per_road(map: &Map, i: &Intersection) -> ControlTrafficSignal {
    let mut ts = new(i.id);

    let sorted_roads = i.get_roads_sorted_by_incoming_angle(map);
    for idx in 0..sorted_roads.len() {
        let r = sorted_roads[idx];
        let adj1 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) - 1);
        let adj2 = *abstutil::wraparound_get(&sorted_roads, (idx as isize) + 1);

        let mut stage = Stage::new();
        for movement in i.movements.values() {
            if movement.turn_type.pedestrian_crossing() {
                if movement.id.from.road == adj1 || movement.id.from.road == adj2 {
                    stage.protected_movements.insert(movement.id);
                }
            } else if movement.id.from.road == r {
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
fn expand_all_stages(ts: &mut ControlTrafficSignal, i: &Intersection) {
    for stage in ts.stages.iter_mut() {
        for g in i.movements.keys() {
            if stage.could_be_protected(*g, i) {
                stage.protected_movements.insert(*g);
            }
        }
    }
}

const PROTECTED: bool = true;
const YIELD: bool = false;

fn make_stages(
    ts: &mut ControlTrafficSignal,
    map_config: &MapConfig,
    i: &Intersection,
    stage_specs: Vec<Vec<(Vec<RoadID>, TurnType, bool)>>,
) {
    for specs in stage_specs {
        let mut stage = Stage::new();
        let mut explicit_crosswalks = false;
        for (roads, mut turn_type, protected) in specs.iter() {
            // The heuristics are written assuming right turns are easy and lefts are hard, so
            // invert in the UK.
            if map_config.driving_side == DrivingSide::Left {
                if turn_type == TurnType::Right {
                    turn_type = TurnType::Left;
                } else if turn_type == TurnType::Left {
                    turn_type = TurnType::Right;
                }
            }
            if turn_type.pedestrian_crossing() {
                explicit_crosswalks = true;
            }

            for movement in i.movements.values() {
                if !roads.contains(&movement.id.from.road) || turn_type != movement.turn_type {
                    continue;
                }

                // If turn on red is banned, ignore movements when the stage has
                // no protected (green) movement from that road
                if !map_config.turn_on_red
                    && !specs.iter().any(|(other_roads, _, other_protected)|
                        *other_protected
                        && other_roads.contains(&movement.id.from.road))
                {
                    continue;
                }

                stage.edit_movement(
                    movement,
                    if *protected {
                        TurnPriority::Protected
                    } else {
                        TurnPriority::Yield
                    },
                );
            }
        }

        // If the specification didn't explicitly include crosswalks, add them in here. Some
        // crosswalks stretch across multiple roads when some parts of a road are missing a
        // sidewalk, so it's hard to specify them in all cases.
        if !explicit_crosswalks {
            // TODO If a stage has no protected turns at all, this adds the crosswalk to multiple
            // stages in a pretty weird way. It'd be better to add to just one stage -- the one
            // with the least conflicting yields.
            for movement in i.movements.values() {
                if movement.turn_type.pedestrian_crossing()
                    && stage.could_be_protected(movement.id, i)
                {
                    stage.edit_movement(movement, TurnPriority::Protected);
                }
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
            info!(
                "Flipping stage order of {} and {} to synchronize them",
                i1, i2
            );
            map.traffic_signals.get_mut(&i1).unwrap().stages.swap(0, 1);
        }
    }
}

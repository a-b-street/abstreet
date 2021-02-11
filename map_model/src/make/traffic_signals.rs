use std::collections::{BTreeSet, HashSet};

use geom::Duration;

use crate::{
    ControlTrafficSignal, DrivingSide, IntersectionCluster, IntersectionID, Map, Movement,
    MovementID, RoadID, Stage, StageType, TurnPriority, TurnType,
};

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
    if let Some(ts) = lagging_green(map, id) {
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

///
/// Create a traffic signal which has a stage that is: protected straight, protected right,
/// unprotected left, unprotected right on red. Followed by a variable stage that has protected
/// left, unprotected right on red. With a last stage that is all-walk and variable.
/// In some degenerate cases, usually with one or more one-way, this can reduce to stage per road.
/// In some rare cases, usually with an alleyway, oncoming lanes can't both be protected left turns.
/// In such cases the stage is split into two stages with each having a protected and yeild turn.
fn lagging_green(map: &Map, id: IntersectionID) -> Option<ControlTrafficSignal> {
    let mut ts = new(id, map);
    if let Some(other) = three_way_three_stage(map, id) {
        ts.stages = other.stages;
    } else if let Some(other) = four_way_four_stage(map, id) {
        ts.stages = other.stages;
    }
    ts.convert_to_ped_scramble_without_promotion();
    // We don't always get a valid traffic signal from the default 3-way and 4-way. When we don't
    // we need to try assembling stages with a more complex algorithm.
    if ts.validate().is_err() {
        if let Some(other) = multi_way_stages(map, id) {
            ts.stages = other.stages;
            ts.convert_to_ped_scramble_without_promotion();
        }
    }
    if let Err(err) = ts.validate() {
        // when all else fails, use stage per road and all-walk stage at the end
        debug!("multi-way validation_error={} ts={:#?}", err, ts);
        ts = stage_per_road(map, id);
        ts.convert_to_ped_scramble();
    }
    // Now, we can optimize the stages, as we're guaranteed to have a valid and complete set of
    // stages.

    // Remove stages which don't contain a protected route.
    ts.stages.retain(|s| !s.protected_movements.is_empty());
    // Determine if any stages can be merged. We could merge turns, but if we end up not reducing
    // the stage as a result, its probably not worth doing, or can be easily added by the user.
    while let Some(merged_ts) = merge_stages(&ts) {
        ts = merged_ts;
    }
    make_lagging_green_variable(&mut ts);
    make_crosswalk_variable(&mut ts);
    return Some(ts);

    // @todo: move all of these internal fn out when refactoring to lagging_green.rs
    // convert walk to variable with a min duration not less than 15 seconds
    fn make_crosswalk_variable(ts: &mut ControlTrafficSignal) {
        const MIN_CROSSWALK_TIME: Duration = Duration::const_seconds(15.0);
        for mut s in ts.stages.iter_mut() {
            if let Some(mut duration) = s.max_crosswalk_time(&ts.movements) {
                match s.stage_type {
                    StageType::Fixed(_) => {
                        // get the minimum duration, but never less than 15 seconds
                        if duration < MIN_CROSSWALK_TIME {
                            duration = MIN_CROSSWALK_TIME;
                        }
                        s.stage_type = StageType::Variable(
                            duration,
                            Duration::const_seconds(1.0),
                            Duration::const_seconds(1.0),
                        )
                    }
                    _ => (),
                }
            }
        }
    }

    fn merge_stages(ts: &ControlTrafficSignal) -> Option<ControlTrafficSignal> {
        for s_src in &ts.stages {
            // s_src is the stage we want to apply to the other stages
            for s_dst in &ts.stages {
                if s_src == s_dst {
                    continue;
                }
                let mut merged_stage = s_dst.clone();
                merged_stage
                    .protected_movements
                    .extend(s_src.protected_movements.clone());

                let mut maybe_ts = ts.clone();
                // insert at the head, keeping crosswalk last
                maybe_ts.stages.insert(0, merged_stage);
                if maybe_ts.validate().is_ok() {
                    let mut stages: Vec<Stage> = Vec::new();
                    for s in maybe_ts.stages {
                        if s != *s_src && s != *s_dst {
                            stages.push(s);
                        }
                    }
                    maybe_ts.stages = stages;
                    return Some(maybe_ts);
                }
            }
        }
        None
    }

    // Sometimes protected oncoming left turns aren't possible.
    fn is_conflict(ts: &ControlTrafficSignal, stage: &Stage) -> Option<(MovementID, MovementID)> {
        for m1 in stage.protected_movements.iter().map(|m| &ts.movements[m]) {
            for m2 in stage.protected_movements.iter().map(|m| &ts.movements[m]) {
                // Use low-level turn conflict, since we know this a road to road movement.
                if m1.id != m2.id && m1.geom.intersection(&m2.geom).is_some() {
                    return Some((m1.id, m2.id));
                }
            }
        }
        return None;
    }

    fn protected_yield_stage(p: MovementID, y: MovementID) -> Stage {
        let mut stage = Stage::new();
        stage.protected_movements.insert(p);
        stage.yield_movements.insert(y);
        stage
    }

    /// Build stages. First find roads that are straight across, they are either one-way or two-way.
    /// For one-way, add any right or left turns, thus completing the stage. For two-way, two
    /// stages will be added. The first stage has protected straight, and right and yield left.
    /// The second stage has protected left. Lastly, sometimes oncomming left turns can't both
    /// be protected, if this occurs the 2nd stage will have one direction protected and the
    /// other yeild and a 3rd, inverse, stage will be added which has the other direction's left
    /// protected and other yield. Finally, any turns which weren't assigned, because there
    /// are no straights or there are more than just pairs of straight intersections, are assigned a
    /// stage. These, too are handled as pairs until one remains, which is handled as a one-way.
    fn multi_way_stages(map: &Map, id: IntersectionID) -> Option<ControlTrafficSignal> {
        let mut ts = new(id, map);
        let (mut right, mut left, straight, mut roads) = movements(&ts);
        let (one_way, two_way) = straight_types(&straight);
        for m in &one_way {
            let mut stage = Stage::new();
            stage.protected_movements.insert(m.clone());
            for t in turns(&m.from.id, &right) {
                stage.protected_movements.insert(t.clone());
            }
            for t in turns(&m.from.id, &left) {
                stage.protected_movements.insert(t.clone());
            }
            add_stage(&mut ts, stage);
            roads.remove(&m.from.id);
        }
        for (m1, m2) in &two_way {
            let mut stage1 = Stage::new();
            let mut stage2 = Stage::new();
            // Insert the straight movements, followed by the right and then the left.
            stage1.protected_movements.insert(m1.clone());
            stage1.protected_movements.insert(m2.clone());
            turns(&m1.from.id, &right).iter().for_each(|t| {
                stage1.protected_movements.insert(*t);
            });
            turns(&m2.from.id, &right).iter().for_each(|t| {
                stage1.protected_movements.insert(*t);
            });
            turns(&m1.from.id, &left).iter().for_each(|t| {
                stage1.yield_movements.insert(*t);
                stage2.protected_movements.insert(*t);
            });
            turns(&m2.from.id, &left).iter().for_each(|t| {
                stage1.yield_movements.insert(*t);
                stage2.protected_movements.insert(*t);
            });
            add_stage(&mut ts, stage1);
            if let Some((m1, m2)) = is_conflict(&ts, &stage2) {
                // We've hit the case where oncoming left turns can't both be protected.
                add_stage(&mut ts, protected_yield_stage(m1, m2));
                add_stage(&mut ts, protected_yield_stage(m2, m1));
            } else {
                add_stage(&mut ts, stage2);
            }
            roads.remove(&m1.from.id);
            roads.remove(&m2.from.id);
        }
        // We may be done assigning, or we may have some roads we haven't dealt with yet.
        let mut vec: Vec<_> = roads.into_iter().collect();
        // We're going to treat the roads as if thery are straight. Otherwise,
        // we'd end up with overlapping protected left turns.
        while let Some(r1) = vec.pop() {
            let mut stage1 = Stage::new();
            let mut stage2 = Stage::new();
            if let Some(r2) = vec.pop() {
                // dual stage, with lagging left turns
                if let Some(m) = remove_movement(&r1, &r2, &mut right) {
                    stage1.protected_movements.insert(m.clone());
                } else if let Some(m) = remove_movement(&r1, &r2, &mut left) {
                    stage1.protected_movements.insert(m.clone());
                }
                if let Some(m) = remove_movement(&r2, &r1, &mut right) {
                    stage1.protected_movements.insert(m.clone());
                } else if let Some(m) = remove_movement(&r2, &r1, &mut left) {
                    stage1.protected_movements.insert(m.clone());
                }

                // add right turns
                turns(&r1, &right).iter().for_each(|t| {
                    stage1.protected_movements.insert(*t);
                });
                turns(&r2, &right).iter().for_each(|t| {
                    stage1.protected_movements.insert(*t);
                });

                // add left turns
                turns(&r1, &left).iter().for_each(|t| {
                    stage1.yield_movements.insert(*t);
                    stage2.protected_movements.insert(*t);
                });
                turns(&r2, &left).iter().for_each(|t| {
                    stage1.yield_movements.insert(*t);
                    stage2.protected_movements.insert(*t);
                });
                // add the stages
                add_stage(&mut ts, stage1);
                add_stage(&mut ts, stage2);
            } else {
                // single stage without lagging left turns
                turns(&r1, &right).iter().for_each(|t| {
                    stage1.protected_movements.insert(*t);
                });
                turns(&r1, &left).iter().for_each(|t| {
                    stage1.protected_movements.insert(*t);
                });
                add_stage(&mut ts, stage1);
            }
        }
        Some(ts)
    }

    fn add_stage(ts: &mut ControlTrafficSignal, stage: Stage) {
        // If there aren't any protected movements, don't add it.
        if stage.protected_movements.is_empty() {
            return;
        }
        // Ensure a duplicate isn't added.
        if ts.stages.iter().all(|s| *s != stage) {
            ts.stages.push(stage)
        }
    }

    fn turns(from: &RoadID, turns: &Vec<MovementID>) -> Vec<MovementID> {
        turns
            .iter()
            .filter_map(|turn| {
                if from.0 == turn.from.id.0 {
                    Some(turn.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    fn remove_movement(
        from: &RoadID,
        to: &RoadID,
        turns: &mut Vec<MovementID>,
    ) -> Option<MovementID> {
        let result = if let Some(m) = turns
            .iter()
            .filter(|turn| from.0 == turn.from.id.0 && to.0 == turn.to.id.0)
            .next()
        {
            Some(m.clone())
        } else {
            None
        };
        if result.is_some() {
            turns.retain(|turn| from.0 != turn.from.id.0 || to.0 != turn.to.id.0);
        }
        result
    }

    fn three_way_three_stage(map: &Map, i: IntersectionID) -> Option<ControlTrafficSignal> {
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

        // Three-stage with protected lefts, right turn on red
        make_stages(
            &mut ts,
            map.config.driving_side,
            vec![
                vec![
                    (vec![north, south], TurnType::Straight, PROTECTED),
                    (vec![north, south], TurnType::Right, PROTECTED),
                    (vec![north, south], TurnType::Left, YIELD),
                    (vec![east], TurnType::Right, YIELD),
                ],
                vec![
                    (vec![north, south], TurnType::Left, PROTECTED),
                    (vec![east], TurnType::Right, YIELD),
                ],
                vec![
                    (vec![east], TurnType::Straight, PROTECTED),
                    (vec![east], TurnType::Right, PROTECTED),
                    (vec![east], TurnType::Left, PROTECTED),
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
                    (vec![north, south], TurnType::Left, YIELD),
                    (vec![north, south], TurnType::Right, PROTECTED),
                    (vec![east, west], TurnType::Right, YIELD),
                ],
                vec![
                    (vec![north, south], TurnType::Left, PROTECTED),
                    (vec![east, west], TurnType::Right, YIELD),
                ],
                vec![
                    (vec![east, west], TurnType::Straight, PROTECTED),
                    (vec![east, west], TurnType::Left, YIELD),
                    (vec![east, west], TurnType::Right, PROTECTED),
                    (vec![north, south], TurnType::Right, YIELD),
                ],
                vec![
                    (vec![east, west], TurnType::Left, PROTECTED),
                    (vec![north, south], TurnType::Right, YIELD),
                ],
            ],
        );
        Some(ts)
    }

    fn movements(
        ts: &ControlTrafficSignal,
    ) -> (
        Vec<MovementID>,
        Vec<MovementID>,
        Vec<MovementID>,
        BTreeSet<RoadID>,
    ) {
        let mut right: Vec<MovementID> = Vec::new();
        let mut left: Vec<MovementID> = Vec::new();
        let mut straight: Vec<MovementID> = Vec::new();
        let mut set: BTreeSet<RoadID> = BTreeSet::new();

        for (id, m) in &ts.movements {
            if !id.crosswalk {
                match m.turn_type {
                    TurnType::Right => right.push(id.clone()),
                    TurnType::Left => left.push(id.clone()),
                    TurnType::Straight => straight.push(id.clone()),
                    _ => (),
                }
                set.insert(id.from.id);
            }
        }
        (right, left, straight, set)
    }

    fn straight_types(
        movements: &Vec<MovementID>,
    ) -> (Vec<MovementID>, Vec<(MovementID, MovementID)>) {
        let mut one_way: Vec<MovementID> = Vec::new();
        let mut two_way: Vec<(MovementID, MovementID)> = Vec::new();
        for m in movements {
            if let Some(other) = movements
                .iter()
                .find(|&other| m.from.id == other.to.id && m.to.id == other.from.id)
            {
                two_way.push((m.clone(), other.clone()));
            } else {
                one_way.push(m.clone());
            }
        }
        (one_way, two_way)
    }

    fn make_lagging_green_variable(ts: &mut ControlTrafficSignal) {
        const EXTENT_DURATION: Duration = Duration::const_seconds(10.0);
        const MAX_DURATION: Duration = Duration::const_seconds(20.0);
        let mut prev_stage: Option<&mut Stage> = None;
        for stage in ts.stages.iter_mut() {
            // Lagging green: if this stage's protected movements were yield movements in the
            // previous stage, make this stage optional.
            if let Some(prev) = prev_stage {
                if stage
                    .protected_movements
                    .iter()
                    .all(|m| prev.yield_movements.contains(m))
                {
                    if let StageType::Fixed(_) = stage.stage_type {
                        stage.stage_type =
                            StageType::Variable(Duration::ZERO, EXTENT_DURATION, MAX_DURATION);
                    }
                }
            }
            prev_stage = Some(stage);
        }
    }
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

/// Temporary experiment to group all movements into the smallest number of stages.
pub fn brute_force(map: &Map, i: IntersectionID) {
    let movements: Vec<Movement> = Movement::for_i(i, map)
        .unwrap()
        .into_iter()
        .filter_map(|(id, m)| if id.crosswalk { None } else { Some(m) })
        .collect();
    let indices: Vec<usize> = (0..movements.len()).collect();
    for num_stages in 1..=movements.len() {
        println!(
            "For {} turn movements, looking for solution with {} stages",
            movements.len(),
            num_stages
        );
        for partition in helper(&indices, num_stages) {
            if okay_partition(movements.iter().collect(), partition) {
                return;
            }
        }
    }
    unreachable!()
}

fn okay_partition(movements: Vec<&Movement>, partition: Partition) -> bool {
    for stage in partition.0 {
        let mut protected: Vec<&Movement> = Vec::new();
        for idx in stage {
            let m = movements[idx];
            if protected.iter().any(|other| m.conflicts_with(other)) {
                return false;
            }
            protected.push(m);
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

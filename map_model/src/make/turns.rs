use crate::{
    Intersection, IntersectionID, IntersectionType, Lane, LaneID, LaneType, Road, RoadID, Turn,
    TurnID, TurnType, LANE_THICKNESS,
};
use abstutil::{Timer, Warn};
use geom::{Distance, Line, PolyLine, Pt2D};
use nbez::{Bez3o, BezCurve, Point2d};
use std::collections::{BTreeSet, HashMap, HashSet};

// TODO Add proper warnings when the geometry is too small to handle.

pub fn make_all_turns(
    i: &Intersection,
    roads: &Vec<Road>,
    lanes: &Vec<Lane>,
    timer: &mut Timer,
) -> Vec<Turn> {
    assert!(i.intersection_type != IntersectionType::Border);

    let mut raw_turns: Vec<Turn> = Vec::new();
    raw_turns.extend(make_vehicle_turns(i, roads, lanes, timer));
    raw_turns.extend(make_walking_turns(i, roads, lanes, timer));
    let unique_turns = ensure_unique(raw_turns);

    let mut final_turns: Vec<Turn> = Vec::new();
    let mut filtered_turns: HashMap<LaneID, Vec<Turn>> = HashMap::new();
    for turn in unique_turns {
        if !does_turn_pass_restrictions(&turn, &i.roads, roads, lanes) {
            continue;
        }

        if is_turn_allowed(&turn, roads, lanes) {
            final_turns.push(turn);
        } else {
            filtered_turns
                .entry(turn.id.src)
                .or_insert_with(Vec::new)
                .push(turn);
        }
    }

    // Make sure every incoming lane has a turn originating from it, and every outgoing lane has a
    // turn leading to it. Except for parking lanes, of course.
    let mut incoming_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.incoming_lanes {
        if lanes[l.0].lane_type != LaneType::Parking {
            incoming_missing.insert(*l);
        }
    }
    for t in &final_turns {
        incoming_missing.remove(&t.id.src);
    }
    // Turn restrictions are buggy. If they orphan a lane, restore the filtered turns.
    for (l, turns) in filtered_turns {
        if incoming_missing.contains(&l) {
            timer.warn(format!(
                "Turn restrictions broke {} outbound, so restoring turns",
                l
            ));
            final_turns.extend(turns);
            incoming_missing.remove(&l);
        }
    }

    let mut outgoing_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.outgoing_lanes {
        if lanes[l.0].lane_type != LaneType::Parking {
            outgoing_missing.insert(*l);
        }
    }
    for t in &final_turns {
        outgoing_missing.remove(&t.id.dst);
    }

    if !incoming_missing.is_empty() || !outgoing_missing.is_empty() {
        timer.warn(format!(
            "Turns for {} orphan some lanes. Incoming: {:?}, outgoing: {:?}",
            i.id, incoming_missing, outgoing_missing
        ));
    }

    final_turns
}

fn ensure_unique(turns: Vec<Turn>) -> Vec<Turn> {
    let mut ids = HashSet::new();
    let mut keep: Vec<Turn> = Vec::new();
    for t in turns.into_iter() {
        if ids.contains(&t.id) {
            panic!("Duplicate turns {}!", t.id);
        } else {
            ids.insert(t.id);
            keep.push(t);
        }
    }
    keep
}

fn make_vehicle_turns(
    i: &Intersection,
    all_roads: &Vec<Road>,
    lanes: &Vec<Lane>,
    timer: &mut Timer,
) -> Vec<Turn> {
    let roads: Vec<&Road> = i.roads.iter().map(|r| &all_roads[r.0]).collect();
    let mut lane_types: BTreeSet<LaneType> = BTreeSet::new();
    for r in &roads {
        let (t1, t2) = r.get_lane_types();
        for lt in t1.into_iter().chain(t2.into_iter()) {
            lane_types.insert(lt);
        }
    }
    lane_types.remove(&LaneType::Parking);
    lane_types.remove(&LaneType::Sidewalk);

    let mut result: Vec<Option<Turn>> = Vec::new();

    for lane_type in lane_types.into_iter() {
        if i.is_dead_end() {
            result
                .extend(make_vehicle_turns_for_dead_end(i, all_roads, lanes, lane_type).get(timer));
            continue;
        }

        for r1 in &roads {
            // We can't filter incoming just on the preferred type, because we might be forced to
            // make a turn from a driving lane to a bike/bus lane.
            let incoming = filter_vehicle_lanes(r1.incoming_lanes(i.id), lane_type);
            if incoming.is_empty() {
                continue;
            }

            let mut maybe_add_turns = Vec::new();
            let mut all_incoming_lanes_covered = false;

            for r2 in &roads {
                if r1.id == r2.id {
                    continue;
                }
                let outgoing = filter_vehicle_lanes(r2.outgoing_lanes(i.id), lane_type);
                if outgoing.is_empty() {
                    continue;
                }

                // If we fell back to driving lanes for both incoming and outgoing and it's not
                // time, then skip. This should prevent duplicates.
                if lanes[incoming[0].0].lane_type != lane_type
                    && lanes[outgoing[0].0].lane_type != lane_type
                {
                    continue;
                }

                // Use an arbitrary lane from each road to get the angle between r1 and r2.
                let angle1 = lanes[incoming[0].0].last_line().angle();
                let angle2 = lanes[outgoing[0].0].first_line().angle();

                match TurnType::from_angles(angle1, angle2) {
                    TurnType::Straight => {
                        // Cartesian product. Additionally detect where the lane-changing movements
                        // happen. But we have to use the indices assuming all travel lanes, not
                        // just the restricted set. :\
                        let all_incoming = r1
                            .incoming_lanes(i.id)
                            .iter()
                            .filter_map(|(id, lt)| {
                                if lt.is_for_moving_vehicles() {
                                    Some(*id)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<LaneID>>();
                        let all_outgoing = r2
                            .outgoing_lanes(i.id)
                            .iter()
                            .filter_map(|(id, lt)| {
                                if lt.is_for_moving_vehicles() {
                                    Some(*id)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<LaneID>>();

                        for (idx1, l1) in all_incoming.into_iter().enumerate() {
                            for (idx2, l2) in all_outgoing.iter().enumerate() {
                                if !incoming.contains(&l1) || !outgoing.contains(l2) {
                                    continue;
                                }
                                if let Some(mut t) = make_vehicle_turn(lanes, i.id, l1, *l2) {
                                    if idx1 < idx2 {
                                        t.turn_type = TurnType::LaneChangeRight;
                                    } else if idx1 > idx2 {
                                        t.turn_type = TurnType::LaneChangeLeft;
                                    }
                                    result.push(Some(t));
                                }
                            }
                        }
                        all_incoming_lanes_covered = true;
                    }
                    TurnType::Right => {
                        for (idx, l1) in incoming.iter().enumerate() {
                            for l2 in &outgoing {
                                let turn = make_vehicle_turn(lanes, i.id, *l1, *l2);
                                if idx == incoming.len() - 1 {
                                    result.push(turn);
                                } else {
                                    maybe_add_turns.push(turn);
                                }
                            }
                        }
                    }
                    TurnType::Left => {
                        for (idx, l1) in incoming.iter().enumerate() {
                            for l2 in &outgoing {
                                let turn = make_vehicle_turn(lanes, i.id, *l1, *l2);
                                if idx == 0 {
                                    result.push(turn);
                                } else {
                                    maybe_add_turns.push(turn);
                                }
                            }
                        }
                    }
                    _ => unreachable!(),
                };
            }

            if !all_incoming_lanes_covered {
                result.extend(maybe_add_turns);
            }
        }
    }

    result.into_iter().filter_map(|x| x).collect()
}

fn make_vehicle_turns_for_dead_end(
    i: &Intersection,
    roads: &Vec<Road>,
    lanes: &Vec<Lane>,
    lane_type: LaneType,
) -> Warn<Vec<Option<Turn>>> {
    let road = &roads[i.roads.iter().next().unwrap().0];
    let incoming = filter_vehicle_lanes(road.incoming_lanes(i.id), lane_type);
    let outgoing = filter_vehicle_lanes(road.outgoing_lanes(i.id), lane_type);
    if incoming.is_empty() || outgoing.is_empty() {
        return Warn::warn(Vec::new(), format!("{} needs to be a border node!", i.id));
    }

    let mut result = Vec::new();
    for l1 in incoming {
        for l2 in &outgoing {
            result.push(make_vehicle_turn(lanes, i.id, l1, *l2));
        }
    }

    Warn::ok(result)
}

fn make_walking_turns(
    i: &Intersection,
    all_roads: &Vec<Road>,
    lanes: &Vec<Lane>,
    timer: &mut Timer,
) -> Vec<Turn> {
    let roads: Vec<&Road> = i
        .get_roads_sorted_by_incoming_angle(all_roads)
        .into_iter()
        .map(|id| &all_roads[id.0])
        .collect();

    let mut result: Vec<Turn> = Vec::new();
    for idx1 in 0..roads.len() {
        if let Some(l1) = get_sidewalk(lanes, roads[idx1].incoming_lanes(i.id)) {
            // Make the crosswalk to the other side
            if let Some(l2) = get_sidewalk(lanes, roads[idx1].outgoing_lanes(i.id)) {
                result.extend(make_crosswalks(i.id, l1, l2));
            }

            // Find the shared corner
            if roads.len() > 1 {
                // TODO -1 and not +1 is brittle... must be the angle sorting
                if let Some(l2) = get_sidewalk(
                    lanes,
                    abstutil::wraparound_get(&roads, (idx1 as isize) - 1).outgoing_lanes(i.id),
                ) {
                    if !l1.last_pt().epsilon_eq(l2.first_pt()) {
                        let geom = make_shared_sidewalk_corner(i, l1, l2, timer);
                        result.push(Turn {
                            id: turn_id(i.id, l1.id, l2.id),
                            turn_type: TurnType::SharedSidewalkCorner,
                            geom: geom.clone(),
                            lookup_idx: 0,
                        });
                        result.push(Turn {
                            id: turn_id(i.id, l2.id, l1.id),
                            turn_type: TurnType::SharedSidewalkCorner,
                            geom: geom.reversed(),
                            lookup_idx: 0,
                        });
                    }
                } else if roads.len() > 2 {
                    // See if we need to add a crosswalk over this adjacent road.
                    // TODO This is brittle; I could imagine having to cross two adjacent highway
                    // ramps to get to the next sidewalk.
                    if let Some(l2) = get_sidewalk(
                        lanes,
                        abstutil::wraparound_get(&roads, (idx1 as isize) - 2).outgoing_lanes(i.id),
                    ) {
                        result.extend(make_crosswalks(i.id, l1, l2));
                    }
                }
            }
        }
    }
    result
}

fn make_crosswalks(i: IntersectionID, l1: &Lane, l2: &Lane) -> Vec<Turn> {
    if l1.last_pt().epsilon_eq(l2.first_pt()) {
        return Vec::new();
    }

    // Jut out a bit into the intersection, cross over, then jut back in.
    let line = Line::new(l1.last_pt(), l2.first_pt()).shift_right(LANE_THICKNESS / 2.0);
    let geom_fwds = PolyLine::new(vec![l1.last_pt(), line.pt1(), line.pt2(), l2.first_pt()]);

    vec![
        Turn {
            id: turn_id(i, l1.id, l2.id),
            turn_type: TurnType::Crosswalk,
            geom: geom_fwds.clone(),
            lookup_idx: 0,
        },
        Turn {
            id: turn_id(i, l2.id, l1.id),
            turn_type: TurnType::Crosswalk,
            geom: geom_fwds.reversed(),
            lookup_idx: 0,
        },
    ]
}

fn make_shared_sidewalk_corner(
    i: &Intersection,
    l1: &Lane,
    l2: &Lane,
    timer: &mut Timer,
) -> PolyLine {
    let baseline = PolyLine::new(vec![l1.last_pt(), l2.first_pt()]);

    // Find all of the points on the intersection polygon between the two sidewalks.
    let corner1 = l1.last_line().shift_right(LANE_THICKNESS / 2.0).pt2();
    let corner2 = l2.first_line().shift_right(LANE_THICKNESS / 2.0).pt1();

    // The order of the points here seems backwards, but it's because we scan from corner2
    // to corner1 below.
    let mut pts_between = vec![l2.first_pt()];
    // Intersection polygons are constructed in clockwise order, so do corner2 to corner1.
    if let Some(pts) =
        Pt2D::find_pts_between(&i.polygon.points(), corner2, corner1, Distance::meters(0.5))
    {
        let deduped = Pt2D::approx_dedupe(pts, geom::EPSILON_DIST);
        if deduped.len() >= 2 {
            if abstutil::contains_duplicates(&deduped.iter().map(|pt| pt.to_hashable()).collect()) {
                timer.warn(format!("SharedSidewalkCorner between {} and {} has weird duplicate geometry, so just doing straight line", l1.id, l2.id));
                return baseline;
            }

            pts_between.extend(
                PolyLine::new(deduped)
                    .shift_right(LANE_THICKNESS / 2.0)
                    .with_context(
                        timer,
                        format!("SharedSidewalkCorner between {} and {}", l1.id, l2.id),
                    )
                    .points(),
            );
        }
    }
    pts_between.push(l1.last_pt());
    pts_between.reverse();
    // Pretty big smoothing; I'm observing funky backtracking about 0.5m long.
    let mut final_pts = Pt2D::approx_dedupe(pts_between.clone(), Distance::meters(1.0));
    if final_pts.len() < 2 {
        timer.warn(format!(
            "SharedSidewalkCorner between {} and {} couldn't do final smoothing",
            l1.id, l2.id
        ));
        final_pts = Pt2D::approx_dedupe(pts_between, geom::EPSILON_DIST);
    }
    // The last point might be removed as a duplicate, but we want the start/end to exactly match
    // up at least.
    if *final_pts.last().unwrap() != l2.first_pt() {
        final_pts.pop();
        final_pts.push(l2.first_pt());
    }
    if abstutil::contains_duplicates(&final_pts.iter().map(|pt| pt.to_hashable()).collect()) {
        timer.warn(format!("SharedSidewalkCorner between {} and {} has weird duplicate geometry, so just doing straight line", l1.id, l2.id));
        return baseline;
    }
    let result = PolyLine::new(final_pts);
    if result.length() > 10.0 * baseline.length() {
        timer.warn(format!("SharedSidewalkCorner between {} and {} explodes to {} long, so just doing straight line", l1.id, l2.id, result.length()));
        return baseline;
    }
    result
}

fn turn_id(parent: IntersectionID, src: LaneID, dst: LaneID) -> TurnID {
    TurnID { parent, src, dst }
}

fn get_sidewalk<'a>(lanes: &'a Vec<Lane>, children: &Vec<(LaneID, LaneType)>) -> Option<&'a Lane> {
    for (id, lt) in children {
        if *lt == LaneType::Sidewalk {
            return Some(&lanes[id.0]);
        }
    }
    None
}

fn filter_vehicle_lanes(lanes: &Vec<(LaneID, LaneType)>, preferred: LaneType) -> Vec<LaneID> {
    let preferred = filter_lanes(lanes, preferred);
    if !preferred.is_empty() {
        return preferred;
    }
    filter_lanes(lanes, LaneType::Driving)
}

fn filter_lanes(lanes: &Vec<(LaneID, LaneType)>, filter: LaneType) -> Vec<LaneID> {
    lanes
        .iter()
        .filter_map(|(id, lt)| if *lt == filter { Some(*id) } else { None })
        .collect()
}

fn make_vehicle_turn(lanes: &Vec<Lane>, i: IntersectionID, l1: LaneID, l2: LaneID) -> Option<Turn> {
    let src = &lanes[l1.0];
    let dst = &lanes[l2.0];
    let turn_type = TurnType::from_angles(src.last_line().angle(), dst.first_line().angle());

    if src.last_pt().epsilon_eq(dst.first_pt()) {
        return None;
    }

    let geom = if turn_type == TurnType::Straight {
        PolyLine::new(vec![src.last_pt(), dst.first_pt()])
    } else {
        // The control points are straight out/in from the source/destination lanes, so
        // that the car exits and enters at the same angle as the road.
        let src_line = src.last_line();
        let dst_line = dst.first_line().reverse();

        // TODO Tune the 5.0 and pieces
        let curve = Bez3o::new(
            to_pt(src.last_pt()),
            to_pt(src_line.unbounded_dist_along(src_line.length() + Distance::meters(5.0))),
            to_pt(dst_line.unbounded_dist_along(dst_line.length() + Distance::meters(5.0))),
            to_pt(dst.first_pt()),
        );
        let pieces = 5;
        PolyLine::new(Pt2D::approx_dedupe(
            (0..=pieces)
                .map(|i| {
                    from_pt(
                        curve
                            .interp(1.0 / f64::from(pieces) * f64::from(i))
                            .unwrap(),
                    )
                })
                .collect(),
            geom::EPSILON_DIST,
        ))
    };

    Some(Turn {
        id: turn_id(i, l1, l2),
        turn_type,
        geom,
        lookup_idx: 0,
    })
}

fn to_pt(pt: Pt2D) -> Point2d<f64> {
    Point2d::new(pt.x(), pt.y())
}

fn from_pt(pt: Point2d<f64>) -> Pt2D {
    Pt2D::new(pt.x, pt.y)
}

fn is_turn_allowed(turn: &Turn, roads: &Vec<Road>, lanes: &Vec<Lane>) -> bool {
    let l = &lanes[turn.id.src.0];
    let r = &roads[l.parent.0];
    if let Some(types) = l.get_turn_restrictions(r) {
        types.contains(&turn.turn_type)
    } else {
        true
    }
}

fn does_turn_pass_restrictions(
    turn: &Turn,
    intersection_roads: &BTreeSet<RoadID>,
    roads: &Vec<Road>,
    lanes: &Vec<Lane>,
) -> bool {
    if turn.between_sidewalks() {
        return true;
    }

    let src = lanes[turn.id.src.0].parent;
    let dst = lanes[turn.id.dst.0].parent;

    for (restriction, to) in &roads[src.0].turn_restrictions {
        // The restriction only applies to one direction of the road.
        if !intersection_roads.contains(to) {
            continue;
        }

        // Ignore the TurnType. Between two roads, there's only one category of TurnType (treating
        // Straight/LaneChangeLeft/LaneChangeRight as the same).
        //
        // Strip off time restrictions (like " @ (Mo-Fr 06:00-09:00, 15:00-18:30)")
        match restriction.split(" @ ").next().unwrap() {
            "no_left_turn" | "no_right_turn" | "no_straight_on" | "no_u_turn" | "no_anything" => {
                if dst == *to {
                    return false;
                }
            }
            "only_left_turn" | "only_right_turn" | "only_straight_on" => {
                if dst != *to {
                    return false;
                }
            }
            _ => panic!("Unknown restriction {}", restriction),
        }
    }

    true
}

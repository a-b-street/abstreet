use crate::raw::{DrivingSide, RestrictionType};
use crate::{
    Intersection, IntersectionID, Lane, LaneID, LaneType, Road, RoadID, Turn, TurnID, TurnType,
};
use abstutil::{wraparound_get, Timer, Warn};
use geom::{Distance, Line, PolyLine, Pt2D, Ring};
use nbez::{Bez3o, BezCurve, Point2d};
use std::collections::{BTreeSet, HashMap, HashSet};

// TODO Add proper warnings when the geometry is too small to handle.

pub fn make_all_turns(
    driving_side: DrivingSide,
    i: &Intersection,
    roads: &Vec<Road>,
    lanes: &Vec<Lane>,
    timer: &mut Timer,
) -> Vec<Turn> {
    assert!(!i.is_border());

    let mut raw_turns: Vec<Turn> = Vec::new();
    raw_turns.extend(make_vehicle_turns(i, roads, lanes, timer));
    raw_turns.extend(make_walking_turns(driving_side, i, roads, lanes, timer));
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
    // turn leading to it.
    let mut incoming_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.incoming_lanes {
        if lanes[l.0].lane_type.supports_any_movement() {
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
        if lanes[l.0].lane_type.supports_any_movement() {
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
            // TODO This was once an assertion, but disabled for
            // https://github.com/dabreegster/abstreet/issues/84. A crosswalk gets created twice
            // and deduplicated here. Not sure why it was double-created in the first place.
            println!("Duplicate turns {}!", t.id);
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
) -> impl Iterator<Item=Turn> {
    let sorted_roads: Vec<&Road> = i
        .get_roads_sorted_by_incoming_angle(all_roads)
        .iter()
        .map(|r| &all_roads[r.0])
        .collect();
    let mut lane_types: BTreeSet<LaneType> = BTreeSet::new();
    for r in &sorted_roads {
        let (t1, t2) = r.get_lane_types();
        for lt in t1.into_iter().chain(t2.into_iter()) {
            lane_types.insert(lt);
        }
    }
    lane_types.remove(&LaneType::LightRail);
    lane_types.remove(&LaneType::Parking);
    lane_types.remove(&LaneType::SharedLeftTurn);
    lane_types.remove(&LaneType::Construction);
    lane_types.remove(&LaneType::Sidewalk);

    let mut result: Vec<Option<Turn>> = Vec::new();

    for lane_type in lane_types.into_iter() {
        if i.roads.len() == 1 {
            result
                .extend(make_vehicle_turns_for_dead_end(i, all_roads, lanes, lane_type).get(timer));
            continue;
        }

        for (idx1, r1) in sorted_roads.iter().enumerate() {
            // We can't filter incoming just on the preferred type, because we might be forced to
            // make a turn from a driving lane to a bike/bus lane.
            let incoming = filter_vehicle_lanes(r1.incoming_lanes(i.id), lane_type);
            if incoming.is_empty() {
                continue;
            }

            let mut maybe_add_turns = Vec::new();
            let mut all_incoming_lanes_covered = false;

            for r2 in &sorted_roads {
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

                let type_from_angle = TurnType::from_angles(angle1, angle2);
                let tt = if type_from_angle == TurnType::Right {
                    // This one's fragile, based on angles. Really we care that there aren't roads
                    // between the two.
                    if wraparound_get(&sorted_roads, (idx1 as isize) - 1).id == r2.id
                        || wraparound_get(&sorted_roads, (idx1 as isize) + 1).id == r2.id
                    {
                        TurnType::Right
                    } else {
                        TurnType::Straight
                    }
                } else {
                    type_from_angle
                };

                match tt {
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
                                if let Some(mut t) = make_vehicle_turn(lanes, i.id, l1, *l2, tt) {
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
                                let turn = make_vehicle_turn(lanes, i.id, *l1, *l2, tt);
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
                                let turn = make_vehicle_turn(lanes, i.id, *l1, *l2, tt);
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

    result.into_iter().filter_map(|x| x)
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
            result.push(make_vehicle_turn(
                lanes,
                i.id,
                l1,
                *l2,
                TurnType::from_angles(
                    lanes[l1.0].last_line().angle(),
                    lanes[l2.0].first_line().angle(),
                ),
            ));
        }
    }

    Warn::ok(result)
}

fn make_walking_turns(
    driving_side: DrivingSide,
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

    // I'm a bit confused when to do -1 and +1 honestly, but this works in practice. Angle sorting
    // may be a little backwards.
    let idx_offset = if driving_side == DrivingSide::Right {
        -1
    } else {
        1
    };

    if roads.len() == 2 {
        if let Some(turns) = make_degenerate_crosswalks(i.id, lanes, roads[0], roads[1]) {
            result.extend(turns);
        }
        // TODO Argh, duplicate logic for SharedSidewalkCorners
        for idx1 in 0..roads.len() {
            if let Some(l1) = get_sidewalk(lanes, roads[idx1].incoming_lanes(i.id)) {
                if let Some(l2) = get_sidewalk(
                    lanes,
                    abstutil::wraparound_get(&roads, (idx1 as isize) + idx_offset)
                        .outgoing_lanes(i.id),
                ) {
                    if l1.last_pt() != l2.first_pt() {
                        let geom = make_shared_sidewalk_corner(driving_side, i, l1, l2, timer);
                        result.push(Turn {
                            id: turn_id(i.id, l1.id, l2.id),
                            turn_type: TurnType::SharedSidewalkCorner,
                            other_crosswalk_ids: BTreeSet::new(),
                            geom: geom.clone(),
                        });
                        result.push(Turn {
                            id: turn_id(i.id, l2.id, l1.id),
                            turn_type: TurnType::SharedSidewalkCorner,
                            other_crosswalk_ids: BTreeSet::new(),
                            geom: geom.reversed(),
                        });
                    }
                }
            }
        }
        return result;
    }
    if roads.len() == 1 {
        if let Some(l1) = get_sidewalk(lanes, roads[0].incoming_lanes(i.id)) {
            if let Some(l2) = get_sidewalk(lanes, roads[0].outgoing_lanes(i.id)) {
                let geom = make_shared_sidewalk_corner(driving_side, i, l1, l2, timer);
                result.push(Turn {
                    id: turn_id(i.id, l1.id, l2.id),
                    turn_type: TurnType::SharedSidewalkCorner,
                    other_crosswalk_ids: BTreeSet::new(),
                    geom: geom.clone(),
                });
                result.push(Turn {
                    id: turn_id(i.id, l2.id, l1.id),
                    turn_type: TurnType::SharedSidewalkCorner,
                    other_crosswalk_ids: BTreeSet::new(),
                    geom: geom.reversed(),
                });
            }
        }
        return result;
    }

    for idx1 in 0..roads.len() {
        if let Some(l1) = get_sidewalk(lanes, roads[idx1].incoming_lanes(i.id)) {
            // Make the crosswalk to the other side
            if let Some(l2) = get_sidewalk(lanes, roads[idx1].outgoing_lanes(i.id)) {
                result.extend(make_crosswalks(i.id, l1, l2));
            }

            // Find the shared corner
            if let Some(l2) = get_sidewalk(
                lanes,
                abstutil::wraparound_get(&roads, (idx1 as isize) + idx_offset).outgoing_lanes(i.id),
            ) {
                if l1.last_pt() != l2.first_pt() {
                    let geom = make_shared_sidewalk_corner(driving_side, i, l1, l2, timer);
                    result.push(Turn {
                        id: turn_id(i.id, l1.id, l2.id),
                        turn_type: TurnType::SharedSidewalkCorner,
                        other_crosswalk_ids: BTreeSet::new(),
                        geom: geom.clone(),
                    });
                    result.push(Turn {
                        id: turn_id(i.id, l2.id, l1.id),
                        turn_type: TurnType::SharedSidewalkCorner,
                        other_crosswalk_ids: BTreeSet::new(),
                        geom: geom.reversed(),
                    });
                }
            } else if let Some(l2) = get_sidewalk(
                lanes,
                abstutil::wraparound_get(&roads, (idx1 as isize) + idx_offset).incoming_lanes(i.id),
            ) {
                // Adjacent road is missing a sidewalk on the near side, but has one on the far
                // side
                result.extend(make_crosswalks(i.id, l1, l2));
            } else {
                // We may need to add a crosswalk over this intermediate road that has no
                // sidewalks at all. There might be a few in the way -- think highway onramps.
                // TODO Refactor and loop until we find something to connect it to?
                if let Some(l2) = get_sidewalk(
                    lanes,
                    abstutil::wraparound_get(&roads, (idx1 as isize) + 2 * idx_offset)
                        .outgoing_lanes(i.id),
                ) {
                    result.extend(make_crosswalks(i.id, l1, l2));
                } else if let Some(l2) = get_sidewalk(
                    lanes,
                    abstutil::wraparound_get(&roads, (idx1 as isize) + 2 * idx_offset)
                        .incoming_lanes(i.id),
                ) {
                    result.extend(make_crosswalks(i.id, l1, l2));
                } else if roads.len() > 3 {
                    if let Some(l2) = get_sidewalk(
                        lanes,
                        abstutil::wraparound_get(&roads, (idx1 as isize) + 3 * idx_offset)
                            .outgoing_lanes(i.id),
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
    let l1_pt = l1.endpoint(i);
    let l2_pt = l2.endpoint(i);
    if l1_pt == l2_pt {
        return Vec::new();
    }
    // TODO Not sure this is always right.
    let direction = if (l1.dst_i == i) == (l2.dst_i == i) {
        -1.0
    } else {
        1.0
    };
    // Jut out a bit into the intersection, cross over, then jut back in. Assumes sidewalks are the
    // same width.
    let line = Line::new(l1_pt, l2_pt).shift_either_direction(direction * l1.width / 2.0);
    let geom_fwds = PolyLine::new(vec![l1_pt, line.pt1(), line.pt2(), l2_pt]);

    vec![
        Turn {
            id: turn_id(i, l1.id, l2.id),
            turn_type: TurnType::Crosswalk,
            other_crosswalk_ids: vec![turn_id(i, l2.id, l1.id)].into_iter().collect(),
            geom: geom_fwds.clone(),
        },
        Turn {
            id: turn_id(i, l2.id, l1.id),
            turn_type: TurnType::Crosswalk,
            other_crosswalk_ids: vec![turn_id(i, l1.id, l2.id)].into_iter().collect(),
            geom: geom_fwds.reversed(),
        },
    ]
}

// Only one physical crosswalk for degenerate intersections, right in the middle.
fn make_degenerate_crosswalks(
    i: IntersectionID,
    lanes: &Vec<Lane>,
    r1: &Road,
    r2: &Road,
) -> Option<impl Iterator<Item=Turn>> {
    let l1_in = get_sidewalk(lanes, r1.incoming_lanes(i))?;
    let l1_out = get_sidewalk(lanes, r1.outgoing_lanes(i))?;
    let l2_in = get_sidewalk(lanes, r2.incoming_lanes(i))?;
    let l2_out = get_sidewalk(lanes, r2.outgoing_lanes(i))?;

    let pt1 = Line::maybe_new(l1_in.last_pt(), l2_out.first_pt())?.percent_along(0.5);
    let pt2 = Line::maybe_new(l1_out.first_pt(), l2_in.last_pt())?.percent_along(0.5);

    if pt1 == pt2 {
        return None;
    }

    let mut all_ids = BTreeSet::new();
    all_ids.insert(turn_id(i, l1_in.id, l1_out.id));
    all_ids.insert(turn_id(i, l1_out.id, l1_in.id));
    all_ids.insert(turn_id(i, l2_in.id, l2_out.id));
    all_ids.insert(turn_id(i, l2_out.id, l2_in.id));

    Some(
        vec![
            Turn {
                id: turn_id(i, l1_in.id, l1_out.id),
                turn_type: TurnType::Crosswalk,
                other_crosswalk_ids: all_ids.clone(),
                geom: PolyLine::new(vec![l1_in.last_pt(), pt1, pt2, l1_out.first_pt()]),
            },
            Turn {
                id: turn_id(i, l1_out.id, l1_in.id),
                turn_type: TurnType::Crosswalk,
                other_crosswalk_ids: all_ids.clone(),
                geom: PolyLine::new(vec![l1_out.first_pt(), pt2, pt1, l1_in.last_pt()]),
            },
            Turn {
                id: turn_id(i, l2_in.id, l2_out.id),
                turn_type: TurnType::Crosswalk,
                other_crosswalk_ids: all_ids.clone(),
                geom: PolyLine::new(vec![l2_in.last_pt(), pt2, pt1, l2_out.first_pt()]),
            },
            Turn {
                id: turn_id(i, l2_out.id, l2_in.id),
                turn_type: TurnType::Crosswalk,
                other_crosswalk_ids: all_ids.clone(),
                geom: PolyLine::new(vec![l2_out.first_pt(), pt1, pt2, l2_in.last_pt()]),
            },
        ]
        .into_iter()
        .map(|mut t| {
            t.other_crosswalk_ids.remove(&t.id);
            t
        }),
    )
}

fn make_shared_sidewalk_corner(
    driving_side: DrivingSide,
    i: &Intersection,
    l1: &Lane,
    l2: &Lane,
    timer: &mut Timer,
) -> PolyLine {
    let baseline = PolyLine::new(vec![l1.last_pt(), l2.first_pt()]);

    // Find all of the points on the intersection polygon between the two sidewalks. Assumes
    // sidewalks are the same length.
    let corner1 = driving_side
        .right_shift_line(l1.last_line(), l1.width / 2.0)
        .pt2();
    let corner2 = driving_side
        .right_shift_line(l2.first_line(), l2.width / 2.0)
        .pt1();

    // TODO Something like this will be MUCH simpler and avoid going around the long way sometimes.
    if false {
        return Ring::new(i.polygon.points().clone()).get_shorter_slice_btwn(corner1, corner2);
    }

    // The order of the points here seems backwards, but it's because we scan from corner2
    // to corner1 below.
    let mut pts_between = vec![l2.first_pt()];
    // Intersection polygons are constructed in clockwise order, so do corner2 to corner1.
    let mut i_pts = i.polygon.points().clone();
    if driving_side == DrivingSide::Left {
        i_pts.reverse();
    }
    if let Some(pts) = Pt2D::find_pts_between(&i_pts, corner2, corner1, Distance::meters(0.5)) {
        let mut deduped = pts.clone();
        deduped.dedup();
        if deduped.len() >= 2 {
            if abstutil::contains_duplicates(&deduped.iter().map(|pt| pt.to_hashable()).collect()) {
                timer.warn(format!(
                    "SharedSidewalkCorner between {} and {} has weird duplicate geometry, so just \
                     doing straight line",
                    l1.id, l2.id
                ));
                return baseline;
            }

            pts_between.extend(
                driving_side
                    .right_shift(PolyLine::new(deduped), l1.width / 2.0)
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
        final_pts = pts_between;
        final_pts.dedup()
    }
    // The last point might be removed as a duplicate, but we want the start/end to exactly match
    // up at least.
    if *final_pts.last().unwrap() != l2.first_pt() {
        final_pts.pop();
        final_pts.push(l2.first_pt());
    }
    if abstutil::contains_duplicates(&final_pts.iter().map(|pt| pt.to_hashable()).collect()) {
        timer.warn(format!(
            "SharedSidewalkCorner between {} and {} has weird duplicate geometry, so just doing \
             straight line",
            l1.id, l2.id
        ));
        return baseline;
    }
    let result = PolyLine::new(final_pts);
    if result.length() > 10.0 * baseline.length() {
        timer.warn(format!(
            "SharedSidewalkCorner between {} and {} explodes to {} long, so just doing straight \
             line",
            l1.id,
            l2.id,
            result.length()
        ));
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

fn make_vehicle_turn(
    lanes: &Vec<Lane>,
    i: IntersectionID,
    l1: LaneID,
    l2: LaneID,
    turn_type: TurnType,
) -> Option<Turn> {
    let src = &lanes[l1.0];
    let dst = &lanes[l2.0];

    if src.last_pt() == dst.first_pt() {
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
        let mut curve: Vec<Pt2D> = (0..=pieces)
            .map(|i| {
                from_pt(
                    curve
                        .interp(1.0 / f64::from(pieces) * f64::from(i))
                        .unwrap(),
                )
            })
            .collect();
        curve.dedup();
        PolyLine::new(curve)
    };

    Some(Turn {
        id: turn_id(i, l1, l2),
        turn_type,
        other_crosswalk_ids: BTreeSet::new(),
        geom,
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
    if let Some(mut types) = l.get_turn_restrictions(r) {
        types.any(|turntype| turntype==turn.turn_type)
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
        match restriction {
            RestrictionType::BanTurns => {
                if dst == *to {
                    return false;
                }
            }
            RestrictionType::OnlyAllowTurns => {
                if dst != *to {
                    return false;
                }
            }
        }
    }

    true
}

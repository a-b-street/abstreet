use std::collections::BTreeSet;

use abstutil::wraparound_get;
use geom::{Distance, Line, PolyLine, Pt2D, Ring};

use crate::{
    Direction, DrivingSide, Intersection, IntersectionID, Lane, LaneID, LaneType, Map, Road, Turn,
    TurnID, TurnType,
};

/// Generate Crosswalk and SharedSidewalkCorner (places where two sidewalks directly meet) turns
pub fn make_walking_turns(map: &Map, i: &Intersection) -> Vec<Turn> {
    if i.merged {
        return make_walking_turns_v2(map, i);
    }

    if i.is_footway(map) {
        return make_footway_turns(map, i);
    }

    let driving_side = map.config.driving_side;

    let roads: Vec<&Road> = i
        .get_roads_sorted_by_incoming_angle(map)
        .into_iter()
        .map(|id| map.get_r(id))
        .collect();
    let mut result: Vec<Turn> = Vec::new();

    // I'm a bit confused when to do -1 and +1 honestly, but this works in practice. Angle sorting
    // may be a little backwards.
    let idx_offset = if driving_side == DrivingSide::Right {
        -1
    } else {
        1
    };

    if i.is_degenerate() {
        if let Some(turns) = make_degenerate_crosswalks(map, i.id, roads[0], roads[1]) {
            result.extend(turns);
        }
        // TODO Argh, duplicate logic for SharedSidewalkCorners
        for idx1 in 0..roads.len() {
            if let Some(l1) = get_sidewalk(map, roads[idx1].incoming_lanes(i.id)) {
                if let Some(l2) = get_sidewalk(
                    map,
                    wraparound_get(&roads, (idx1 as isize) + idx_offset).outgoing_lanes(i.id),
                ) {
                    if l1.last_pt() != l2.first_pt() {
                        let geom = make_shared_sidewalk_corner(driving_side, i, l1, l2);
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
        if let Some(l1) = get_sidewalk(map, roads[0].incoming_lanes(i.id)) {
            if let Some(l2) = get_sidewalk(map, roads[0].outgoing_lanes(i.id)) {
                let geom = make_shared_sidewalk_corner(driving_side, i, l1, l2);
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
        if let Some(l1) = get_sidewalk(map, roads[idx1].incoming_lanes(i.id)) {
            // Make the crosswalk to the other side
            if let Some(l2) = get_sidewalk(map, roads[idx1].outgoing_lanes(i.id)) {
                result.extend(
                    make_crosswalks(i.id, l1, l2, driving_side)
                        .into_iter()
                        .flatten(),
                );
            }

            // Find the shared corner
            if let Some(l2) = get_sidewalk(
                map,
                wraparound_get(&roads, (idx1 as isize) + idx_offset).outgoing_lanes(i.id),
            ) {
                if l1.last_pt() != l2.first_pt() {
                    let geom = make_shared_sidewalk_corner(driving_side, i, l1, l2);
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
                map,
                wraparound_get(&roads, (idx1 as isize) + idx_offset).incoming_lanes(i.id),
            ) {
                // Adjacent road is missing a sidewalk on the near side, but has one on the far
                // side
                result.extend(
                    make_crosswalks(i.id, l1, l2, driving_side)
                        .into_iter()
                        .flatten(),
                );
            } else {
                // We may need to add a crosswalk over this intermediate road that has no
                // sidewalks at all. There might be a few in the way -- think highway onramps.
                // TODO Refactor and loop until we find something to connect it to?
                if let Some(l2) = get_sidewalk(
                    map,
                    wraparound_get(&roads, (idx1 as isize) + 2 * idx_offset).outgoing_lanes(i.id),
                ) {
                    result.extend(
                        make_crosswalks(i.id, l1, l2, driving_side)
                            .into_iter()
                            .flatten(),
                    );
                } else if let Some(l2) = get_sidewalk(
                    map,
                    wraparound_get(&roads, (idx1 as isize) + 2 * idx_offset).incoming_lanes(i.id),
                ) {
                    result.extend(
                        make_crosswalks(i.id, l1, l2, driving_side)
                            .into_iter()
                            .flatten(),
                    );
                } else if roads.len() > 3 {
                    if let Some(l2) = get_sidewalk(
                        map,
                        wraparound_get(&roads, (idx1 as isize) + 3 * idx_offset)
                            .outgoing_lanes(i.id),
                    ) {
                        result.extend(
                            make_crosswalks(i.id, l1, l2, driving_side)
                                .into_iter()
                                .flatten(),
                        );
                    }
                }
            }
        }
    }

    result
}

/// Filter out crosswalks on really short roads. In reality, these roads are usually located within
/// an intersection, which isn't a valid place for a pedestrian crossing.
pub fn filter_turns(mut input: Vec<Turn>, map: &Map, i: &Intersection) -> Vec<Turn> {
    for r in &i.roads {
        if map.get_r(*r).is_extremely_short() {
            input.retain(|t| {
                !(t.id.src.road == *r && t.id.dst.road == *r && t.turn_type == TurnType::Crosswalk)
            });
        }
    }

    input
}

/// A complete rewrite of make_walking_turns, which looks at all sidewalks (or lack thereof) in
/// counter-clockwise order around an intersection. Based on adjacency, create a
/// SharedSidewalkCorner or a Crosswalk.
///
/// TODO This is only used for consolidated intersections right now. Cut over to this completely
/// after fixing problems like:
/// - too many crosswalks at the Boyer roundabout
/// - one centered crosswalk for degenerate intersections
fn make_walking_turns_v2(map: &Map, i: &Intersection) -> Vec<Turn> {
    let driving_side = map.config.driving_side;

    // Consider all roads in counter-clockwise order. Every road has up to two sidewalks. Gather
    // those in order, remembering what roads don't have them.
    let mut lanes: Vec<Option<&Lane>> = Vec::new();
    let mut num_sidewalks = 0;
    let mut sorted_roads = i.get_roads_sorted_by_incoming_angle(map);
    // And for left-handed driving, we need to walk around in the opposite order.
    if driving_side == DrivingSide::Left {
        sorted_roads.reverse();
    }

    for r in sorted_roads {
        let road = map.get_r(r);
        let mut fwd = None;
        let mut back = None;
        for l in &road.lanes {
            if l.lane_type.is_walkable() {
                if l.dir == Direction::Fwd {
                    fwd = Some(l);
                } else {
                    back = Some(l);
                }
            }
        }
        if fwd.is_some() {
            num_sidewalks += 1;
        }
        if back.is_some() {
            num_sidewalks += 1;
        }
        let (in_lane, out_lane) = if road.src_i == i.id {
            (back, fwd)
        } else {
            (fwd, back)
        };
        lanes.push(in_lane);
        lanes.push(out_lane);
    }
    if num_sidewalks <= 1 {
        return Vec::new();
    }
    // Make sure we start with a sidewalk.
    while lanes[0].is_none() {
        lanes.rotate_left(1);
    }
    let mut result: Vec<Turn> = Vec::new();

    let mut from: Option<&Lane> = lanes[0];
    let first_from = from.unwrap().id;
    let mut adj = true;
    for l in lanes.iter().skip(1).chain(lanes.iter()) {
        if i.id.0 == 284 {
            debug!(
                "looking at {:?}. from is {:?}, first_from is {}, adj is {}",
                l.map(|l| l.id),
                from.map(|l| l.id),
                first_from,
                adj
            );
        }

        if from.is_none() {
            from = *l;
            adj = true;
            continue;
        }
        let l1 = from.unwrap();

        if l.is_none() {
            adj = false;
            continue;
        }
        let l2 = l.unwrap();

        if adj && l1.id.road != l2.id.road {
            // Because of the order we go, have to swap l1 and l2 here. l1 is the outgoing, l2 the
            // incoming.
            let geom = make_shared_sidewalk_corner(driving_side, i, l2, l1);
            result.push(Turn {
                id: turn_id(i.id, l1.id, l2.id),
                turn_type: TurnType::SharedSidewalkCorner,
                other_crosswalk_ids: BTreeSet::new(),
                geom: geom.reversed(),
            });
            result.push(Turn {
                id: turn_id(i.id, l2.id, l1.id),
                turn_type: TurnType::SharedSidewalkCorner,
                other_crosswalk_ids: BTreeSet::new(),
                geom,
            });

            from = Some(l2);
        // adj stays true
        } else {
            // TODO Just one for degenerate intersections
            result.extend(
                make_crosswalks(i.id, l1, l2, driving_side)
                    .into_iter()
                    .flatten(),
            );
            from = Some(l2);
            adj = true;
        }

        // Have we made it all the way around?
        if first_from == from.unwrap().id {
            break;
        }
    }

    result
}

/// At an intersection of footpaths only, just generate a turn between every pair of lanes.
fn make_footway_turns(map: &Map, i: &Intersection) -> Vec<Turn> {
    let lanes = i
        .incoming_lanes
        .iter()
        .chain(&i.outgoing_lanes)
        .filter_map(|l| {
            let l = map.get_l(*l);
            if l.is_walkable() {
                Some(l)
            } else {
                None
            }
        })
        .collect::<Vec<&Lane>>();
    let mut results = Vec::new();
    for l1 in &lanes {
        for l2 in &lanes {
            if l1.id == l2.id {
                continue;
            }
            let maybe_geom = PolyLine::new(vec![l1.endpoint(i.id), l2.endpoint(i.id)]);
            let geom = maybe_geom.unwrap_or_else(|_| {
                // TODO Gross! After improving intersection geometry where these cases are
                // happening, if this still happens, maybe it's time to make turn geometry be
                // optional.
                PolyLine::must_new(vec![l1.endpoint(i.id), l1.endpoint(i.id).offset(0.1, 0.1)])
            });
            results.push(Turn {
                id: turn_id(i.id, l1.id, l2.id),
                turn_type: TurnType::SharedSidewalkCorner,
                other_crosswalk_ids: BTreeSet::new(),
                geom,
            });
        }
    }
    results
}

fn make_crosswalks(
    i: IntersectionID,
    l1: &Lane,
    l2: &Lane,
    driving_side: DrivingSide,
) -> Option<Vec<Turn>> {
    let l1_pt = l1.endpoint(i);
    let l2_pt = l2.endpoint(i);
    // This is one of those uncomfortably "trial-and-error" kind of things.
    let mut direction = if (l1.dst_i == i) == (l2.dst_i == i) {
        -1.0
    } else {
        1.0
    };
    if driving_side == DrivingSide::Left {
        direction *= -1.0;
    }

    // Jut out a bit into the intersection, cross over, then jut back in. Assumes sidewalks are the
    // same width.
    let line = Line::new(l1_pt, l2_pt)?.shift_either_direction(direction * l1.width / 2.0);
    let geom_fwds = PolyLine::deduping_new(vec![l1_pt, line.pt1(), line.pt2(), l2_pt]).ok()?;

    Some(vec![
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
    ])
}

// Only one physical crosswalk for degenerate intersections, right in the middle.
fn make_degenerate_crosswalks(
    map: &Map,
    i: IntersectionID,
    r1: &Road,
    r2: &Road,
) -> Option<Vec<Turn>> {
    let l1_in = get_sidewalk(map, r1.incoming_lanes(i))?;
    let l1_out = get_sidewalk(map, r1.outgoing_lanes(i))?;
    let l2_in = get_sidewalk(map, r2.incoming_lanes(i))?;
    let l2_out = get_sidewalk(map, r2.outgoing_lanes(i))?;

    let pt1 = Line::new(l1_in.last_pt(), l2_out.first_pt())?.percent_along(0.5)?;
    let pt2 = Line::new(l1_out.first_pt(), l2_in.last_pt())?.percent_along(0.5)?;

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
                geom: PolyLine::deduping_new(vec![l1_in.last_pt(), pt1, pt2, l1_out.first_pt()])
                    .ok()?,
            },
            Turn {
                id: turn_id(i, l1_out.id, l1_in.id),
                turn_type: TurnType::Crosswalk,
                other_crosswalk_ids: all_ids.clone(),
                geom: PolyLine::deduping_new(vec![l1_out.first_pt(), pt2, pt1, l1_in.last_pt()])
                    .ok()?,
            },
            Turn {
                id: turn_id(i, l2_in.id, l2_out.id),
                turn_type: TurnType::Crosswalk,
                other_crosswalk_ids: all_ids.clone(),
                geom: PolyLine::deduping_new(vec![l2_in.last_pt(), pt2, pt1, l2_out.first_pt()])
                    .ok()?,
            },
            Turn {
                id: turn_id(i, l2_out.id, l2_in.id),
                turn_type: TurnType::Crosswalk,
                other_crosswalk_ids: all_ids,
                geom: PolyLine::deduping_new(vec![l2_out.first_pt(), pt1, pt2, l2_in.last_pt()])
                    .ok()?,
            },
        ]
        .into_iter()
        .map(|mut t| {
            t.other_crosswalk_ids.remove(&t.id);
            t
        })
        .collect(),
    )
}

// TODO This doesn't handle sidewalk/shoulder transitions
fn make_shared_sidewalk_corner(
    driving_side: DrivingSide,
    i: &Intersection,
    l1: &Lane,
    l2: &Lane,
) -> PolyLine {
    let baseline = PolyLine::must_new(vec![l1.last_pt(), l2.first_pt()]);

    // Find all of the points on the intersection polygon between the two sidewalks. Assumes
    // sidewalks are the same length.
    let corner1 = l1.last_line().shift_right(l1.width / 2.0).pt2();
    let corner2 = l2.first_line().shift_right(l2.width / 2.0).pt1();

    // TODO Something like this will be MUCH simpler and avoid going around the long way sometimes.
    if false {
        return Ring::must_new(i.polygon.points().clone())
            .get_shorter_slice_btwn(corner1, corner2)
            .unwrap();
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
        let mut deduped = pts;
        deduped.dedup();
        if deduped.len() >= 2 {
            if abstutil::contains_duplicates(
                &deduped
                    .iter()
                    .map(|pt| pt.to_hashable())
                    .collect::<Vec<_>>(),
            ) {
                warn!(
                    "SharedSidewalkCorner between {} and {} has weird duplicate geometry, so just \
                     doing straight line",
                    l1.id, l2.id
                );
                return baseline;
            }

            if let Ok(pl) = PolyLine::must_new(deduped).shift_right(l1.width.min(l2.width) / 2.0) {
                pts_between.extend(pl.points());
            } else {
                warn!(
                    "SharedSidewalkCorner between {} and {} has weird collapsing geometry, so \
                     just doing straight line",
                    l1.id, l2.id
                );
                return baseline;
            }
        }
    }
    pts_between.push(l1.last_pt());
    pts_between.reverse();
    // Pretty big smoothing; I'm observing funky backtracking about 0.5m long.
    let mut final_pts = Pt2D::approx_dedupe(pts_between.clone(), Distance::meters(1.0));
    if final_pts.len() < 2 {
        warn!(
            "SharedSidewalkCorner between {} and {} couldn't do final smoothing",
            l1.id, l2.id
        );
        final_pts = pts_between;
        final_pts.dedup()
    }
    // The last point might be removed as a duplicate, but we want the start/end to exactly match
    // up at least.
    if *final_pts.last().unwrap() != l2.first_pt() {
        final_pts.pop();
        final_pts.push(l2.first_pt());
    }
    if abstutil::contains_duplicates(
        &final_pts
            .iter()
            .map(|pt| pt.to_hashable())
            .collect::<Vec<_>>(),
    ) {
        warn!(
            "SharedSidewalkCorner between {} and {} has weird duplicate geometry, so just doing \
             straight line",
            l1.id, l2.id
        );
        return baseline;
    }
    let result = PolyLine::must_new(final_pts);
    if result.length() > 10.0 * baseline.length() {
        warn!(
            "SharedSidewalkCorner between {} and {} explodes to {} long, so just doing straight \
             line",
            l1.id,
            l2.id,
            result.length()
        );
        return baseline;
    }
    result
}

fn turn_id(parent: IntersectionID, src: LaneID, dst: LaneID) -> TurnID {
    TurnID { parent, src, dst }
}

fn get_sidewalk(map: &Map, children: Vec<(LaneID, LaneType)>) -> Option<&Lane> {
    for (id, lt) in children {
        if lt.is_walkable() {
            return Some(map.get_l(id));
        }
    }
    None
}

use crate::raw::DrivingSide;
use crate::{Intersection, IntersectionID, Lane, LaneID, LaneType, Road, Turn, TurnID, TurnType};
use abstutil::{wraparound_get, Timer};
use geom::{Distance, Line, PolyLine, Pt2D, Ring};
use std::collections::BTreeSet;

pub fn make_walking_turns(
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
                    wraparound_get(&roads, (idx1 as isize) + idx_offset).outgoing_lanes(i.id),
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
                result.extend(make_crosswalks(i.id, l1, l2).into_iter().flatten());
            }

            // Find the shared corner
            if let Some(l2) = get_sidewalk(
                lanes,
                wraparound_get(&roads, (idx1 as isize) + idx_offset).outgoing_lanes(i.id),
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
                wraparound_get(&roads, (idx1 as isize) + idx_offset).incoming_lanes(i.id),
            ) {
                // Adjacent road is missing a sidewalk on the near side, but has one on the far
                // side
                result.extend(make_crosswalks(i.id, l1, l2).into_iter().flatten());
            } else {
                // We may need to add a crosswalk over this intermediate road that has no
                // sidewalks at all. There might be a few in the way -- think highway onramps.
                // TODO Refactor and loop until we find something to connect it to?
                if let Some(l2) = get_sidewalk(
                    lanes,
                    wraparound_get(&roads, (idx1 as isize) + 2 * idx_offset).outgoing_lanes(i.id),
                ) {
                    result.extend(make_crosswalks(i.id, l1, l2).into_iter().flatten());
                } else if let Some(l2) = get_sidewalk(
                    lanes,
                    wraparound_get(&roads, (idx1 as isize) + 2 * idx_offset).incoming_lanes(i.id),
                ) {
                    result.extend(make_crosswalks(i.id, l1, l2).into_iter().flatten());
                } else if roads.len() > 3 {
                    if let Some(l2) = get_sidewalk(
                        lanes,
                        wraparound_get(&roads, (idx1 as isize) + 3 * idx_offset)
                            .outgoing_lanes(i.id),
                    ) {
                        result.extend(make_crosswalks(i.id, l1, l2).into_iter().flatten());
                    }
                }
            }
        }
    }

    result
}

fn make_crosswalks(i: IntersectionID, l1: &Lane, l2: &Lane) -> Option<Vec<Turn>> {
    let l1_pt = l1.endpoint(i);
    let l2_pt = l2.endpoint(i);
    // TODO Not sure this is always right.
    let direction = if (l1.dst_i == i) == (l2.dst_i == i) {
        -1.0
    } else {
        1.0
    };
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
    i: IntersectionID,
    lanes: &Vec<Lane>,
    r1: &Road,
    r2: &Road,
) -> Option<impl Iterator<Item = Turn>> {
    let l1_in = get_sidewalk(lanes, r1.incoming_lanes(i))?;
    let l1_out = get_sidewalk(lanes, r1.outgoing_lanes(i))?;
    let l2_in = get_sidewalk(lanes, r2.incoming_lanes(i))?;
    let l2_out = get_sidewalk(lanes, r2.outgoing_lanes(i))?;

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
                other_crosswalk_ids: all_ids.clone(),
                geom: PolyLine::deduping_new(vec![l2_out.first_pt(), pt1, pt2, l2_in.last_pt()])
                    .ok()?,
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
    let baseline = PolyLine::must_new(vec![l1.last_pt(), l2.first_pt()]);

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
        return Ring::must_new(i.polygon.points().clone()).get_shorter_slice_btwn(corner1, corner2);
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
                    .right_shift(PolyLine::must_new(deduped), l1.width / 2.0)
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
    let result = PolyLine::must_new(final_pts);
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

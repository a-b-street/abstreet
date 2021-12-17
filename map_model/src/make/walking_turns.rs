use geom::{Distance, PolyLine, Pt2D, Ring};

use crate::{
    Direction, DrivingSide, Intersection, IntersectionID, Lane, LaneID, Map, Turn, TurnID, TurnType,
};

/// Looks at all sidewalks (or lack thereof) in counter-clockwise order around an intersection.
/// Based on adjacency, create a SharedSidewalkCorner or a Crosswalk.
/// UnmarkedCrossings are not generated here; another process later "downgrades" crosswalks to
/// unmarked.
pub fn make_walking_turns(map: &Map, i: &Intersection) -> Vec<Turn> {
    let driving_side = map.config.driving_side;

    // Consider all roads in counter-clockwise order. Every road has up to two sidewalks. Gather
    // those in order, remembering what roads don't have them.
    let mut lanes: Vec<Option<&Lane>> = Vec::new();
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

        let (in_lane, out_lane) = if road.src_i == i.id {
            (back, fwd)
        } else {
            (fwd, back)
        };

        // Don't add None entries for footways even if they only have one lane
        if map.get_r(r).is_footway() {
            if in_lane.is_some() {
                lanes.push(in_lane);
            }
            if out_lane.is_some() {
                lanes.push(out_lane);
            }
        } else {
            lanes.push(in_lane);
            lanes.push(out_lane);
        }
    }

    // If there are 0 or 1 sidewalks there are no turns to be made
    if lanes.iter().filter(|l| l.is_some()).count() <= 1 {
        return Vec::new();
    }

    // At a deadend make only one SharedSidewalkCorner
    if i.is_deadend() {
        let (l1, l2) = (lanes[0].unwrap(), lanes[1].unwrap());
        return vec![Turn {
            id: turn_id(i.id, l1.id, l2.id),
            turn_type: TurnType::SharedSidewalkCorner,
            geom: make_shared_sidewalk_corner(i, l1, l2),
        }];
    }

    // Make sure we start with a sidewalk.
    while lanes[0].is_none() {
        lanes.rotate_left(1);
    }
    let mut result: Vec<Turn> = Vec::new();

    let mut from: Option<&Lane> = lanes[0];
    let mut adj = true;
    for l in lanes.iter().skip(1).chain(lanes.iter().take(1)) {
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
            result.push(Turn {
                id: turn_id(i.id, l1.id, l2.id),
                turn_type: TurnType::SharedSidewalkCorner,
                geom: make_shared_sidewalk_corner(i, l1, l2),
            });

            from = Some(l2);
        // adj stays true
        } else {
            result.push(Turn {
                id: turn_id(i.id, l1.id, l2.id),
                turn_type: TurnType::Crosswalk,
                geom: make_crosswalk(i, l1, l2),
            });
            from = Some(l2);
            adj = true;
        }
    }

    // If there are exactly two crosswalks they must be connected, so delete one.
    if result
        .iter()
        .filter(|t| t.turn_type == TurnType::Crosswalk)
        .count()
        == 2
    {
        result.remove(
            result
                .iter()
                .position(|t| t.turn_type == TurnType::Crosswalk)
                .unwrap(),
        );
    }

    result
}

/// Filter out crosswalks on really short roads. In reality, these roads are usually located within
/// an intersection, which isn't a valid place for a pedestrian crossing.
///
/// And if the road is marked as having no crosswalks at an end, downgrade them to unmarked
/// crossings.
pub fn filter_turns(mut input: Vec<Turn>, map: &Map, i: &Intersection) -> Vec<Turn> {
    for r in &i.roads {
        if map.get_r(*r).is_extremely_short() {
            input.retain(|t| {
                !(t.id.src.road == *r && t.id.dst.road == *r && t.turn_type.pedestrian_crossing())
            });
        }
    }

    for turn in &mut input {
        if let Some(dr) = turn.crosswalk_over_road(map) {
            let road = map.get_r(dr.road);
            let keep = if dr.dir == Direction::Fwd {
                road.crosswalk_forward
            } else {
                road.crosswalk_backward
            };
            if !keep {
                turn.turn_type = TurnType::UnmarkedCrossing;
            }
        } else if turn.turn_type.pedestrian_crossing() {
            // We have a crosswalk over multiple roads (or sometimes, just one road that only has a
            // walkable lane on one side of it). We can't yet detect all the roads crossed. So for
            // now, it's more often correct to assume that if any nearby roads don't have a
            // crossing snapped to both ends, then there's probably no crosswalk here.
            for l in [turn.id.src, turn.id.dst] {
                let road = map.get_parent(l);
                if !road.crosswalk_forward || !road.crosswalk_backward {
                    turn.turn_type = TurnType::UnmarkedCrossing;
                }
            }
        }
    }

    input
}

fn make_crosswalk(i: &Intersection, l1: &Lane, l2: &Lane) -> PolyLine {
    let l1_line = l1.end_line(i.id);
    let l2_line = l2.end_line(i.id);

    // Jut out a bit into the intersection, cross over, then jut back in.
    // Put degenerate intersection crosswalks in the middle (DEGENERATE_HALF_LENGTH).
    PolyLine::deduping_new(vec![
        l1_line.pt2(),
        l1_line.unbounded_dist_along(
            l1_line.length()
                + if i.is_degenerate() {
                    Distance::const_meters(2.5)
                } else {
                    l1.width / 2.0
                },
        ),
        l2_line.unbounded_dist_along(
            l2_line.length()
                + if i.is_degenerate() {
                    Distance::const_meters(2.5)
                } else {
                    l2.width / 2.0
                },
        ),
        l2_line.pt2(),
    ])
    .unwrap_or_else(|_| PolyLine::unchecked_new(vec![l1.endpoint(i.id), l2.endpoint(i.id)]))
}

// TODO This doesn't handle sidewalk/shoulder transitions
fn make_shared_sidewalk_corner(i: &Intersection, l1: &Lane, l2: &Lane) -> PolyLine {
    // This may produce a polyline with two identical points. Nothing better to do here.
    let baseline = PolyLine::unchecked_new(vec![l1.endpoint(i.id), l2.endpoint(i.id)]);

    // Is point2 counter-clockwise of point1?
    let dir = if i
        .polygon
        .center()
        .angle_to(l1.endpoint(i.id))
        .simple_shortest_rotation_towards(i.polygon.center().angle_to(l2.endpoint(i.id)))
        > 0.0
    {
        1.0
    } else {
        -1.0
    // For deadends, go the long way around
    } * if i.is_deadend() { -1.0 } else { 1.0 };
    // Find all of the points on the intersection polygon between the two sidewalks. Assumes
    // sidewalks are the same length.
    let corner1 = l1
        .end_line(i.id)
        .shift_either_direction(dir * l1.width / 2.0)
        .pt2();
    let corner2 = l2
        .end_line(i.id)
        .shift_either_direction(-dir * l2.width / 2.0)
        .pt2();

    // TODO Something like this will be MUCH simpler and avoid going around the long way sometimes.
    if false {
        return Ring::must_new(i.polygon.points().clone())
            .get_shorter_slice_btwn(corner1, corner2)
            .unwrap();
    }

    // The order of the points here seems backwards, but it's because we scan from corner2
    // to corner1 below.

    let mut pts_between = vec![l2.endpoint(i.id)];
    // Intersection polygons are constructed in clockwise order, so do corner2 to corner1.
    let mut i_pts = i.polygon.points().clone();

    if dir < 0.0 {
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

            if let Ok(pl) = PolyLine::must_new(deduped)
                .shift_either_direction(dir * l1.width.min(l2.width) / 2.0)
            {
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
    pts_between.push(l1.endpoint(i.id));
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
    if *final_pts.last().unwrap() != l2.endpoint(i.id) {
        final_pts.pop();
        final_pts.push(l2.endpoint(i.id));
    }
    if abstutil::contains_duplicates(
        &final_pts
            .iter()
            .map(|pt| pt.to_hashable())
            .collect::<Vec<_>>(),
    ) || final_pts.len() < 2
    {
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

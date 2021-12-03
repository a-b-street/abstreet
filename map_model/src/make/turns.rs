use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::Result;
use nbez::{Bez3o, BezCurve, Point2d};

use geom::{Angle, Distance, Line, PolyLine, Pt2D};

use crate::{Intersection, Lane, LaneID, LaneType, Map, RoadID, Turn, TurnID, TurnType};

/// Generate all driving and walking turns at an intersection, accounting for OSM turn restrictions.
pub fn make_all_turns(map: &Map, i: &Intersection) -> Vec<Turn> {
    let mut raw_turns: Vec<Turn> = Vec::new();
    raw_turns.extend(make_vehicle_turns(i, map));
    raw_turns.extend(crate::make::walking_turns::filter_turns(
        crate::make::walking_turns::make_walking_turns(map, i),
        map,
        i,
    ));
    let unique_turns = ensure_unique(raw_turns);
    // Never allow turns that go against road-level turn restrictions; that upstream OSM data is
    // usually not extremely broken.
    let all_turns: Vec<Turn> = unique_turns
        .into_iter()
        .filter(|t| t.permitted_by_road(i, map))
        .collect();

    // Try to use turn lane tags...
    let filtered_turns: Vec<Turn> = all_turns
        .clone()
        .into_iter()
        .filter(|t| t.permitted_by_lane(map))
        .collect();
    // And remove merging left or right turns. If we wanted to remove the "lane-changing at
    // intersections" behavior, we could do this for TurnType::Straight too.
    let filtered_turns = remove_merging_turns(map, filtered_turns, TurnType::Right);
    let mut filtered_turns = remove_merging_turns(map, filtered_turns, TurnType::Left);
    if i.merged {
        filtered_turns.retain(|turn| {
            if turn.turn_type == TurnType::UTurn {
                let src_lane = map.get_l(turn.id.src);
                // U-turns at divided highways are sometimes legal (and a common movement --
                // https://www.openstreetmap.org/way/361443212), so let OSM turn:lanes override.
                if src_lane
                    .get_lane_level_turn_restrictions(map.get_r(src_lane.id.road), false)
                    .map(|set| !set.contains(&TurnType::UTurn))
                    .unwrap_or(true)
                {
                    warn!("Removing u-turn from merged intersection: {}", turn.id);
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });
    }

    // But then see how all of that filtering affects lane connectivity.
    match verify_vehicle_connectivity(&filtered_turns, i, map) {
        Ok(()) => filtered_turns,
        Err(err) => {
            warn!("Not filtering turns. {}", err);
            all_turns
        }
    }
}

fn ensure_unique(turns: Vec<Turn>) -> Vec<Turn> {
    let mut ids = HashSet::new();
    let mut keep: Vec<Turn> = Vec::new();
    for t in turns.into_iter() {
        if ids.contains(&t.id) {
            // TODO This was once an assertion, but disabled for
            // https://github.com/a-b-street/abstreet/issues/84. A crosswalk gets created twice
            // and deduplicated here. Not sure why it was double-created in the first place.
            warn!("Duplicate turns {}!", t.id);
        } else {
            ids.insert(t.id);
            keep.push(t);
        }
    }
    keep
}

/// Ideally, we want every incoming lane to lead to at least one lane of the same type, and every
/// outgoing lane to be reachable by at least one lane of the same type. But if it's a bus or bike
/// lane, settle for being connected to anything -- even just a driving lane. There's naturally
/// places where these dedicated lanes start and end.
///
/// Why is this definition strict for driving lanes connected to other driving lanes?  See
/// https://www.openstreetmap.org/node/491979474 for a motivating example. When a dedicated bike
/// path crosses a road with turn restrictions marked on a segment before the intersection, the
/// turn restrictions _probably_ indicate the vehicle movements allowed further on, and _don't_
/// describe the turns between the road and the trail.
pub fn verify_vehicle_connectivity(turns: &[Turn], i: &Intersection, map: &Map) -> Result<()> {
    let mut incoming_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.incoming_lanes {
        if map.get_l(*l).lane_type.is_for_moving_vehicles() {
            incoming_missing.insert(*l);
        }
    }
    let mut outgoing_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.outgoing_lanes {
        if map.get_l(*l).lane_type.is_for_moving_vehicles() {
            outgoing_missing.insert(*l);
        }
    }

    for turn in turns {
        let src_lt = map.get_l(turn.id.src).lane_type;
        let dst_lt = map.get_l(turn.id.dst).lane_type;

        if src_lt == dst_lt {
            incoming_missing.remove(&turn.id.src);
            outgoing_missing.remove(&turn.id.dst);
        }

        if src_lt == LaneType::Biking || src_lt == LaneType::Bus {
            incoming_missing.remove(&turn.id.src);
        }
        if dst_lt == LaneType::Biking || dst_lt == LaneType::Bus {
            outgoing_missing.remove(&turn.id.dst);
        }
    }

    if !incoming_missing.is_empty() || !outgoing_missing.is_empty() {
        bail!(
            "Turns for {} orphan some lanes. Incoming: {:?}, outgoing: {:?}",
            i.id,
            incoming_missing,
            outgoing_missing
        );
    }
    Ok(())
}

fn make_vehicle_turns(i: &Intersection, map: &Map) -> Vec<Turn> {
    let mut turns = Vec::new();

    let expected_turn_types = expected_turn_types_for_four_way(i, map);

    // Just generate every possible combination of turns between incoming and outgoing lanes.
    let is_deadend = i.roads.len() == 1;
    for src in &i.incoming_lanes {
        let src = map.get_l(*src);
        if !src.lane_type.is_for_moving_vehicles() {
            continue;
        }
        for dst in &i.outgoing_lanes {
            let dst = map.get_l(*dst);
            if !dst.lane_type.is_for_moving_vehicles() {
                continue;
            }
            // Only allow U-turns at deadends
            if src.id.road == dst.id.road && !is_deadend {
                continue;
            }
            // Can't go between light rail and normal roads
            if src.is_light_rail() != dst.is_light_rail() {
                continue;
            }
            if src.last_pt() == dst.first_pt() {
                warn!(
                    "No turn from {} to {}; the endpoints are the same",
                    src.id, dst.id
                );
                continue;
            }

            let from_angle = src.last_line().angle();
            let to_angle = dst.first_line().angle();
            let mut turn_type = turn_type_from_angles(from_angle, to_angle);
            if turn_type == TurnType::UTurn {
                // Lots of false positives when classifying these just based on angles. So also
                // require the road names to match.
                if map.get_parent(src.id).get_name(None) != map.get_parent(dst.id).get_name(None) {
                    // Distinguish really sharp lefts/rights based on clockwiseness
                    if from_angle.simple_shortest_rotation_towards(to_angle) < 0.0 {
                        turn_type = TurnType::Right;
                    } else {
                        turn_type = TurnType::Left;
                    }
                }

                // Some service roads wind up very short. Allowing u-turns there causes vehicles to
                // gridlock pretty much instantly, because they occupy two intersections during the
                // attempted movement.
                if is_deadend && src.length() < Distance::meters(7.0) {
                    warn!("Skipping U-turn at tiny deadend on {}", src.id);
                    continue;
                }
            } else if let Some(expected_type) = expected_turn_types
                .as_ref()
                .and_then(|e| e.get(&(src.id.road, dst.id.road)))
            {
                // At some 4-way intersections, roads meet at strange angles, throwing off
                // turn_type_from_angles. Correct it based on relative ordering.
                if turn_type != *expected_type {
                    warn!(
                        "Turn from {} to {} looks like {:?} by angle, but is {:?} by ordering",
                        src.id.road, dst.id.road, turn_type, expected_type
                    );
                    turn_type = *expected_type;
                }
            }

            let geom = if turn_type == TurnType::Straight {
                PolyLine::must_new(vec![src.last_pt(), dst.first_pt()])
            } else {
                curvey_turn(src, dst)
                    .unwrap_or_else(|_| PolyLine::must_new(vec![src.last_pt(), dst.first_pt()]))
            };

            turns.push(Turn {
                id: TurnID {
                    parent: i.id,
                    src: src.id,
                    dst: dst.id,
                },
                turn_type,
                geom,
            });
        }
    }

    turns
}

fn curvey_turn(src: &Lane, dst: &Lane) -> Result<PolyLine> {
    // The control points are straight out/in from the source/destination lanes, so
    // that the car exits and enters at the same angle as the road.
    let src_line = src.last_line();
    let dst_line = dst.first_line().reversed();

    // TODO Tune the 5.0 and pieces
    let pt1 = src.last_pt();
    let control_pt1 = src_line.unbounded_dist_along(src_line.length() + Distance::meters(5.0));
    let control_pt2 = dst_line.unbounded_dist_along(dst_line.length() + Distance::meters(5.0));
    let pt2 = dst.first_pt();

    // If the intersection is too small, the endpoints and control points squish together, and
    // they'll overlap. In that case, just use the straight line for the turn.
    if let (Some(l1), Some(l2)) = (Line::new(pt1, control_pt1), Line::new(control_pt2, pt2)) {
        if l1.crosses(&l2) {
            bail!("intersection is too small for a Bezier curve");
        }
    }

    let curve = Bez3o::new(
        to_pt(pt1),
        to_pt(control_pt1),
        to_pt(control_pt2),
        to_pt(pt2),
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
}

fn to_pt(pt: Pt2D) -> Point2d<f64> {
    Point2d::new(pt.x(), pt.y())
}

fn from_pt(pt: Point2d<f64>) -> Pt2D {
    Pt2D::new(pt.x, pt.y)
}

fn remove_merging_turns(map: &Map, input: Vec<Turn>, turn_type: TurnType) -> Vec<Turn> {
    let mut turns = Vec::new();

    // Group turns of the specified type by (from, to) road
    let mut pairs: BTreeMap<(RoadID, RoadID), Vec<Turn>> = BTreeMap::new();
    for t in input {
        // Only do this for driving lanes
        if !map.get_l(t.id.src).is_driving() || !map.get_l(t.id.dst).is_driving() {
            turns.push(t);
            continue;
        }

        if t.turn_type == turn_type {
            pairs
                .entry((t.id.src.road, t.id.dst.road))
                .or_insert_with(Vec::new)
                .push(t);
        } else {
            // Other turn types always pass through
            turns.push(t);
        }
    }

    for (_, group) in pairs {
        if group.len() == 1 {
            turns.extend(group);
            continue;
        }

        let num_src_lanes = group.iter().map(|t| t.id.src).collect::<HashSet<_>>().len();
        let num_dst_lanes = group.iter().map(|t| t.id.dst).collect::<HashSet<_>>().len();

        // Allow all turns from one to many
        if num_src_lanes == 1 {
            turns.extend(group);
            continue;
        }

        // If the number of source and destination lanes is the same, match them up in order,
        // without any crossing.
        if num_src_lanes == num_dst_lanes {
            // But we want to match things up -- leftmost turn lane leads to leftmost destination.
            let mut src_lanes_in_order: Vec<LaneID> = group.iter().map(|t| t.id.src).collect();
            src_lanes_in_order.sort_by_key(|l| map.get_parent(*l).dir_and_offset(*l).1);
            let mut dst_lanes_in_order: Vec<LaneID> = group.iter().map(|t| t.id.dst).collect();
            dst_lanes_in_order.sort_by_key(|l| map.get_parent(*l).dir_and_offset(*l).1);

            for t in group {
                let src_order = src_lanes_in_order
                    .iter()
                    .position(|l| t.id.src == *l)
                    .unwrap();
                let dst_order = dst_lanes_in_order
                    .iter()
                    .position(|l| t.id.dst == *l)
                    .unwrap();
                if src_order == dst_order {
                    turns.push(t);
                }
            }
            continue;
        }

        // If src < dst and src isn't 1, then one source lane gets to access multiple destination
        // lanes. For now, just give up figuring this out, and allow all combinations.
        //
        // TODO https://wiki.openstreetmap.org/wiki/Relation:connectivity may have hints about a
        // better algorithm.
        if num_src_lanes < num_dst_lanes {
            turns.extend(group);
            continue;
        }

        // If we get here, then multiple source lanes are forced to merge into one destination
        // lane.
        //
        // Just kind of give up on these cases for now, and fall-back to only allowing the leftmost
        // or rightmost source lane to make these turns.
        //
        // That left or rightmost lane can turn into all lanes on the destination road. Tempting to
        // remove this, but it may remove some valid U-turn movements (like on Mercer).
        let road = map.get_parent(group[0].id.src);
        let src = if turn_type == TurnType::Right {
            group
                .iter()
                .max_by_key(|t| road.dir_and_offset(t.id.src).1)
                .unwrap()
                .id
                .src
        } else if turn_type == TurnType::Left {
            group
                .iter()
                .min_by_key(|t| road.dir_and_offset(t.id.src).1)
                .unwrap()
                .id
                .src
        } else {
            unreachable!()
        };
        for t in group {
            if t.id.src == src {
                turns.push(t);
            }
        }
    }
    turns
}

fn turn_type_from_angles(from: Angle, to: Angle) -> TurnType {
    let diff = from.simple_shortest_rotation_towards(to);
    // This is a pretty arbitrary parameter, but a difference of 30 degrees seems reasonable for
    // some observed cases.
    if diff.abs() < 30.0 {
        TurnType::Straight
    } else if diff.abs() > 135.0 {
        TurnType::UTurn
    } else if diff < 0.0 {
        // Clockwise rotation
        TurnType::Right
    } else {
        // Counter-clockwise rotation
        TurnType::Left
    }
}

fn expected_turn_types_for_four_way(
    i: &Intersection,
    map: &Map,
) -> Option<HashMap<(RoadID, RoadID), TurnType>> {
    let roads = i.get_sorted_incoming_roads(map);
    if roads.len() != 4 {
        return None;
    }

    // Just based on relative ordering around the intersection, turns (from road, to road, should
    // have this type)
    let mut expected_turn_types: HashMap<(RoadID, RoadID), TurnType> = HashMap::new();
    for (offset, turn_type) in [
        (1, TurnType::Left),
        (2, TurnType::Straight),
        (3, TurnType::Right),
    ] {
        for from_idx in 0..roads.len() {
            let to = *abstutil::wraparound_get(&roads, (from_idx as isize) + offset);
            expected_turn_types.insert((roads[from_idx], to), turn_type);
        }
    }
    Some(expected_turn_types)
}

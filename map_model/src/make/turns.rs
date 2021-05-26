use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::Result;
use nbez::{Bez3o, BezCurve, Point2d};

use geom::{Angle, Distance, Line, PolyLine, Pt2D};

use crate::raw::RestrictionType;
use crate::{Intersection, Lane, LaneID, Map, RoadID, Turn, TurnID, TurnType};

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

    let mut final_turns: Vec<Turn> = Vec::new();
    let mut filtered_turns: HashMap<LaneID, Vec<Turn>> = HashMap::new();
    for turn in unique_turns {
        if !does_turn_pass_restrictions(&turn, i, map) {
            continue;
        }

        if is_turn_allowed(&turn, map) {
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
        if map.get_l(*l).lane_type.supports_any_movement() {
            incoming_missing.insert(*l);
        }
    }
    for t in &final_turns {
        incoming_missing.remove(&t.id.src);
    }
    for (l, turns) in filtered_turns {
        // Do turn restrictions orphan a lane?
        if incoming_missing.contains(&l) {
            // Restrictions on turn lanes may sometimes actually be more like change:lanes
            // (https://wiki.openstreetmap.org/wiki/Key:change). Try to interpret them that way
            // here, choosing one turn from a bunch of options.

            // If all the turns go to a single road, then ignore the turn type.
            let dst_r = map.get_l(turns[0].id.dst).parent;
            let single_group: Vec<Turn> =
                if turns.iter().all(|t| map.get_l(t.id.dst).parent == dst_r) {
                    turns.clone()
                } else {
                    // Fall back to preferring all the straight turns
                    turns
                        .iter()
                        .filter(|t| t.turn_type == TurnType::Straight)
                        .cloned()
                        .collect()
                };
            if !single_group.is_empty() {
                // Just pick one, with the lowest lane-changing cost. Not using Turn's penalty()
                // here, because
                // 1) We haven't populated turns yet, so from_idx won't work
                // 2) It counts from the right, but I think we actually want to count from the left
                let best = single_group
                    .into_iter()
                    .min_by_key(|t| lc_penalty(t, map))
                    .unwrap();
                final_turns.push(best);
                info!(
                    "Restricted lane-changing on approach to turn lanes at {}",
                    l
                );
            } else {
                warn!("Turn restrictions broke {} outbound, so restoring turns", l);
                final_turns.extend(turns);
            }
            incoming_missing.remove(&l);
        }
    }

    final_turns = remove_merging_turns(map, final_turns, TurnType::Right);
    final_turns = remove_merging_turns(map, final_turns, TurnType::Left);

    if i.merged {
        final_turns.retain(|turn| {
            if turn.turn_type == TurnType::UTurn {
                warn!("Removing u-turn from merged intersection: {}", turn.id);
                false
            } else {
                true
            }
        });
    }

    let mut outgoing_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.outgoing_lanes {
        if map.get_l(*l).lane_type.supports_any_movement() {
            outgoing_missing.insert(*l);
        }
    }
    for t in &final_turns {
        outgoing_missing.remove(&t.id.dst);
    }

    if !incoming_missing.is_empty() || !outgoing_missing.is_empty() {
        warn!(
            "Turns for {} orphan some lanes. Incoming: {:?}, outgoing: {:?}",
            i.id, incoming_missing, outgoing_missing
        );
    }

    final_turns
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

fn is_turn_allowed(turn: &Turn, map: &Map) -> bool {
    if let Some(types) = map
        .get_l(turn.id.src)
        .get_turn_restrictions(map.get_parent(turn.id.src))
    {
        types.contains(&turn.turn_type)
    } else {
        true
    }
}

fn does_turn_pass_restrictions(turn: &Turn, i: &Intersection, map: &Map) -> bool {
    if turn.between_sidewalks() {
        return true;
    }

    let src = map.get_parent(turn.id.src);
    let dst = map.get_l(turn.id.dst).parent;

    for (restriction, to) in &src.turn_restrictions {
        // The restriction only applies to one direction of the road.
        if !i.roads.contains(to) {
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

/// Every incoming lane must lead to at least one lane of the same type. Every outgoing lane
/// must be reachable by at least one lane of the same type.
///
/// Why the same type?
/// See https://www.openstreetmap.org/node/491979474 for a motivating example. When a dedicated
/// bike path crosses a road with turn restrictions marked on a segment before the intersection,
/// the turn restrictions _probably_ indicate the vehicle movements allowed further on, and
/// _don't_ describe the turns between the road and the trail.
pub fn verify_vehicle_connectivity(turns: &Vec<Turn>, i: &Intersection, map: &Map) -> Result<()> {
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
        if map.get_l(turn.id.src).lane_type == map.get_l(turn.id.dst).lane_type {
            incoming_missing.remove(&turn.id.src);
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
            if src.parent == dst.parent && !is_deadend {
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
                .and_then(|e| e.get(&(src.parent, dst.parent)))
            {
                // At some 4-way intersections, roads meet at strange angles, throwing off
                // turn_type_from_angles. Correct it based on relative ordering.
                if turn_type != *expected_type {
                    warn!(
                        "Turn from {} to {} looks like {:?} by angle, but is {:?} by ordering",
                        src.parent, dst.parent, turn_type, expected_type
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
                other_crosswalk_ids: BTreeSet::new(),
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
    let dst_line = dst.first_line().reverse();

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

fn lc_penalty(t: &Turn, map: &Map) -> isize {
    let from = map.get_l(t.id.src);
    let to = map.get_l(t.id.dst);

    let from_idx = {
        let mut cnt = 0;
        let r = map.get_r(from.parent);
        for (l, lt) in r.children(from.dir) {
            if from.lane_type != lt {
                continue;
            }
            cnt += 1;
            if from.id == l {
                break;
            }
        }
        cnt
    };

    let to_idx = {
        let mut cnt = 0;
        let r = map.get_r(to.parent);
        for (l, lt) in r.children(to.dir) {
            if to.lane_type != lt {
                continue;
            }
            cnt += 1;
            if to.id == l {
                break;
            }
        }
        cnt
    };

    ((from_idx as isize) - (to_idx as isize)).abs()
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
                .entry((map.get_l(t.id.src).parent, map.get_l(t.id.dst).parent))
                .or_insert_with(Vec::new)
                .push(t);
        } else {
            turns.push(t);
        }
    }

    for (_, group) in pairs {
        if group.len() == 1 {
            turns.extend(group);
            continue;
        }

        // From one to many is fine
        if group.iter().map(|t| t.id.src).collect::<HashSet<_>>().len() == 1 {
            turns.extend(group);
            continue;
        }

        // We have multiple lanes all with a turn to the same destination road. Most likely, only
        // the rightmost or leftmost can actually make the turn.
        // TODO If OSM turn restrictions explicitly have something like "left|left|", then there
        // are multiple source lanes!
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

        // That left or rightmost lane can turn into all lanes on the destination road. Tempting to
        // remove this, but it may remove some valid U-turn movements (like on Mercer).
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
    for &(offset, turn_type) in &[
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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::Result;
use nbez::{Bez3o, BezCurve, Point2d};

use abstutil::Timer;
use geom::{Angle, Distance, PolyLine, Pt2D};

use crate::raw::RestrictionType;
use crate::{Intersection, Lane, LaneID, Map, RoadID, Turn, TurnID, TurnType};

/// Generate all driving and walking turns at an intersection, accounting for OSM turn restrictions.
pub fn make_all_turns(map: &Map, i: &Intersection, timer: &mut Timer) -> Vec<Turn> {
    let mut raw_turns: Vec<Turn> = Vec::new();
    raw_turns.extend(make_vehicle_turns(i, map, timer));
    raw_turns.extend(crate::make::walking_turns::make_walking_turns(
        map, i, timer,
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
                timer.note(format!(
                    "Restricted lane-changing on approach to turn lanes at {}",
                    l
                ));
            } else {
                timer.warn(format!(
                    "Turn restrictions broke {} outbound, so restoring turns",
                    l
                ));
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
                timer.warn(format!(
                    "Removing u-turn from merged intersection: {}",
                    turn.id
                ));
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

fn make_vehicle_turns(i: &Intersection, map: &Map, timer: &mut Timer) -> Vec<Turn> {
    let mut turns = Vec::new();

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
                timer.warn(format!(
                    "No turn from {} to {}; the endpoints are the same",
                    src.id, dst.id
                ));
                continue;
            }

            let turn_type =
                turn_type_from_angles(src.last_line().angle(), dst.first_line().angle());
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
        for (l, lt) in r.children(r.dir(from.id)) {
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
        for (l, lt) in r.children(r.dir(to.id)) {
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

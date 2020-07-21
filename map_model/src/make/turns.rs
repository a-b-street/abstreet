use crate::raw::{DrivingSide, RestrictionType};
use crate::{Intersection, Lane, LaneID, Road, RoadID, Turn, TurnID, TurnType};
use abstutil::Timer;
use geom::{Distance, PolyLine, Pt2D};
use nbez::{Bez3o, BezCurve, Point2d};
use std::collections::{BTreeSet, HashMap, HashSet};

pub fn make_all_turns(
    driving_side: DrivingSide,
    i: &Intersection,
    roads: &Vec<Road>,
    lanes: &Vec<Lane>,
    timer: &mut Timer,
) -> Vec<Turn> {
    assert!(!i.is_border());

    let mut raw_turns: Vec<Turn> = Vec::new();
    raw_turns.extend(make_vehicle_turns(i, lanes, timer));
    raw_turns.extend(crate::make::walking_turns::make_walking_turns(
        driving_side,
        i,
        roads,
        lanes,
        timer,
    ));
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

fn is_turn_allowed(turn: &Turn, roads: &Vec<Road>, lanes: &Vec<Lane>) -> bool {
    let l = &lanes[turn.id.src.0];
    let r = &roads[l.parent.0];
    if let Some(mut types) = l.get_turn_restrictions(r) {
        types.any(|turn_type| turn_type == turn.turn_type)
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

fn make_vehicle_turns(i: &Intersection, lanes: &Vec<Lane>, timer: &mut Timer) -> Vec<Turn> {
    let mut turns = Vec::new();

    // Just generate every possible combination of turns between incoming and outgoing lanes.
    let is_deadend = i.roads.len() == 1;
    for src in &i.incoming_lanes {
        let src = &lanes[src.0];
        if !src.lane_type.is_for_moving_vehicles() {
            continue;
        }
        for dst in &i.outgoing_lanes {
            let dst = &lanes[dst.0];
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
                TurnType::from_angles(src.last_line().angle(), dst.first_line().angle());
            let geom = if turn_type == TurnType::Straight {
                PolyLine::must_new(vec![src.last_pt(), dst.first_pt()])
            } else {
                curvey_turn(src, dst)
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

fn curvey_turn(src: &Lane, dst: &Lane) -> PolyLine {
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
    PolyLine::must_new(curve)
}

fn to_pt(pt: Pt2D) -> Point2d<f64> {
    Point2d::new(pt.x(), pt.y())
}

fn from_pt(pt: Point2d<f64>) -> Pt2D {
    Pt2D::new(pt.x, pt.y)
}

use crate::raw::{DrivingSide, RestrictionType};
use crate::{
    Intersection, IntersectionID, Lane, LaneID, LaneType, Road, RoadID, Turn, TurnID, TurnType,
};
use abstutil::{wraparound_get, Timer};
use geom::{Distance, PolyLine, Pt2D};
use nbez::{Bez3o, BezCurve, Point2d};
use std::collections::{BTreeSet, HashMap, HashSet};

// create all possible cartesian product combos
// pare down based on LTs. if bike->bike, then rm bike->driving and driving->bike.
// pare down based on leftmost/rightmost
// pare down based on all the OSM things
// refine straight -> LCing

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
    raw_turns.extend(make_vehicle_turns(i, roads, lanes));
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

fn make_vehicle_turns(
    i: &Intersection,
    all_roads: &Vec<Road>,
    lanes: &Vec<Lane>,
) -> impl Iterator<Item = Turn> {
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
    lane_types.remove(&LaneType::Parking);
    lane_types.remove(&LaneType::SharedLeftTurn);
    lane_types.remove(&LaneType::Construction);
    lane_types.remove(&LaneType::Sidewalk);

    let mut result: Vec<Option<Turn>> = Vec::new();

    for lane_type in lane_types.into_iter() {
        if i.roads.len() == 1 {
            result.extend(make_vehicle_turns_for_dead_end(
                i, all_roads, lanes, lane_type,
            ));
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
) -> Vec<Option<Turn>> {
    let road = &roads[i.roads.iter().next().unwrap().0];
    let incoming = filter_vehicle_lanes(road.incoming_lanes(i.id), lane_type);
    let outgoing = filter_vehicle_lanes(road.outgoing_lanes(i.id), lane_type);
    if incoming.is_empty() || outgoing.is_empty() {
        println!("{} needs to be a border node!", i.id);
        return Vec::new();
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

    result
}

fn turn_id(parent: IntersectionID, src: LaneID, dst: LaneID) -> TurnID {
    TurnID { parent, src, dst }
}

fn filter_vehicle_lanes(lanes: &Vec<(LaneID, LaneType)>, preferred: LaneType) -> Vec<LaneID> {
    let list = filter_lanes(lanes, preferred);
    if !list.is_empty() || preferred == LaneType::LightRail {
        return list;
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
        PolyLine::must_new(vec![src.last_pt(), dst.first_pt()])
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
        PolyLine::must_new(curve)
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

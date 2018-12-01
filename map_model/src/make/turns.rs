use abstutil::wraparound_get;
use geom::{Angle, Line};
use std::collections::{BTreeSet, HashSet};
use std::iter;
use {
    Intersection, IntersectionID, IntersectionType, Lane, LaneID, LaneType, Map, Road, RoadID,
    Turn, TurnAngle, TurnID, TurnType,
};

pub fn make_all_turns(i: &Intersection, map: &Map) -> Vec<Turn> {
    if i.intersection_type == IntersectionType::Border {
        return Vec::new();
    }

    let mut turns: Vec<Turn> = Vec::new();
    turns.extend(make_vehicle_turns(i, map));
    turns.extend(make_walking_turns(i, map));
    let turns = dedupe(turns);

    // Make sure every incoming lane has a turn originating from it, and every outgoing lane has a
    // turn leading to it. Except for parking lanes, of course.
    let mut incoming_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.incoming_lanes {
        if map.get_l(*l).lane_type != LaneType::Parking {
            incoming_missing.insert(*l);
        }
    }
    let mut outgoing_missing: HashSet<LaneID> = HashSet::new();
    for l in &i.outgoing_lanes {
        if map.get_l(*l).lane_type != LaneType::Parking {
            outgoing_missing.insert(*l);
        }
    }
    for t in &turns {
        incoming_missing.remove(&t.id.src);
        outgoing_missing.remove(&t.id.dst);
    }
    if !incoming_missing.is_empty() || !outgoing_missing.is_empty() {
        // TODO Annoying, but this error is noisy for border nodes.
        error!(
            "Turns for {} orphan some lanes. Incoming: {:?}, outgoing: {:?}",
            i.id, incoming_missing, outgoing_missing
        );
    }

    turns
}

fn dedupe(turns: Vec<Turn>) -> Vec<Turn> {
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

fn make_vehicle_turns(i: &Intersection, map: &Map) -> Vec<Turn> {
    let roads: Vec<&Road> = i.roads.iter().map(|r| map.get_r(*r)).collect();
    let mut lane_types: BTreeSet<LaneType> = BTreeSet::new();
    for r in &roads {
        let (t1, t2) = r.get_lane_types();
        for lt in t1.into_iter().chain(t2.into_iter()) {
            lane_types.insert(lt);
        }
    }
    lane_types.remove(&LaneType::Parking);
    lane_types.remove(&LaneType::Sidewalk);

    let mut result = Vec::new();

    for lane_type in lane_types.into_iter() {
        if i.is_dead_end() {
            result.extend(make_vehicle_turns_for_dead_end(i, map, lane_type));
            continue;
        }

        for r1 in &roads {
            // We can't filter incoming just on the preferred type, because we might be forced to
            // make a turn from a driving lane to a bike/bus lane.
            let incoming = filter_vehicle_lanes(r1.incoming_lanes(i.id), lane_type);
            if incoming.is_empty() {
                continue;
            }

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
                if map.get_l(incoming[0]).lane_type != lane_type
                    && map.get_l(outgoing[0]).lane_type != lane_type
                {
                    continue;
                }

                // Use an arbitrary lane from each road to get the angle between r1 and r2.
                let angle1 = map.get_l(incoming[0]).last_line().angle();
                let angle2 = map.get_l(outgoing[0]).first_line().angle();
                match TurnAngle::new(angle1, angle2) {
                    TurnAngle::Straight => {
                        // Match up based on the relative number of lanes.
                        result.extend(match_up_lanes(map, i.id, &incoming, &outgoing));
                    }
                    TurnAngle::Right => {
                        result.push(make_vehicle_turn(
                            map,
                            i.id,
                            *incoming.last().unwrap(),
                            *outgoing.last().unwrap(),
                        ));
                    }
                    TurnAngle::Left => {
                        result.push(make_vehicle_turn(map, i.id, incoming[0], outgoing[0]));
                    }
                };
            }
        }
    }

    result
}

fn match_up_lanes(
    map: &Map,
    i: IntersectionID,
    incoming: &Vec<LaneID>,
    outgoing: &Vec<LaneID>,
) -> Vec<Turn> {
    let mut result = Vec::new();
    if incoming.len() < outgoing.len() {
        // Arbitrarily use the leftmost incoming lane to handle the excess.
        let padded_incoming: Vec<&LaneID> = iter::repeat(&incoming[0])
            .take(outgoing.len() - incoming.len())
            .chain(incoming.iter())
            .collect();
        assert_eq!(padded_incoming.len(), outgoing.len());
        for (l1, l2) in padded_incoming.iter().zip(outgoing.iter()) {
            result.push(make_vehicle_turn(map, i, **l1, *l2));
        }
    } else if incoming.len() > outgoing.len() {
        // TODO For non-dead-ends: Ideally if the left/rightmost lanes are for turning, use the
        // unused one to go straight.
        // But for now, arbitrarily use the leftmost outgoing road to handle the excess.
        let padded_outgoing: Vec<&LaneID> = iter::repeat(&outgoing[0])
            .take(incoming.len() - outgoing.len())
            .chain(outgoing.iter())
            .collect();
        assert_eq!(padded_outgoing.len(), incoming.len());
        for (l1, l2) in incoming.iter().zip(&padded_outgoing) {
            result.push(make_vehicle_turn(map, i, *l1, **l2));
        }
    } else {
        // The easy case!
        for (l1, l2) in incoming.iter().zip(outgoing.iter()) {
            result.push(make_vehicle_turn(map, i, *l1, *l2));
        }
    }
    result
}

fn make_vehicle_turns_for_dead_end(i: &Intersection, map: &Map, lane_type: LaneType) -> Vec<Turn> {
    let road = map.get_r(*i.roads.iter().next().unwrap());
    let incoming = filter_vehicle_lanes(road.incoming_lanes(i.id), lane_type);
    let outgoing = filter_vehicle_lanes(road.outgoing_lanes(i.id), lane_type);
    if incoming.is_empty() || outgoing.is_empty() {
        error!("{} needs to be a border node!", i.id);
        return Vec::new();
    }

    match_up_lanes(map, i.id, &incoming, &outgoing)
}

fn make_walking_turns(i: &Intersection, map: &Map) -> Vec<Turn> {
    // Sort roads by the angle into the intersection, so we can reason about sidewalks of adjacent
    // roads.
    let mut roads: Vec<(RoadID, Angle)> = i
        .roads
        .iter()
        .map(|id| {
            let r = map.get_r(*id);

            if r.src_i == i.id {
                (r.id, r.center_pts.reversed().last_line().angle())
            } else if r.dst_i == i.id {
                (r.id, r.center_pts.last_line().angle())
            } else {
                panic!(
                    "Incident road {} doesn't have an endpoint at {}",
                    r.id, i.id
                );
            }
        }).collect();
    roads.sort_by_key(|(_, angle)| angle.normalized_degrees() as i64);

    let mut result: Vec<Turn> = Vec::new();

    for idx1 in 0..roads.len() as isize {
        if let Some(l1) = get_incoming_sidewalk(map, i.id, wraparound_get(&roads, idx1).0) {
            // Make the crosswalk to the other side
            if let Some(l2) = get_outgoing_sidewalk(map, i.id, wraparound_get(&roads, idx1).0) {
                result.push(Turn {
                    id: turn_id(i.id, l1.id, l2.id),
                    turn_type: TurnType::Crosswalk,
                    line: Line::new(l1.last_pt(), l2.first_pt()),
                    lookup_idx: 0,
                });
                result.push(Turn {
                    id: turn_id(i.id, l2.id, l1.id),
                    turn_type: TurnType::Crosswalk,
                    line: Line::new(l2.first_pt(), l1.last_pt()),
                    lookup_idx: 0,
                });
            }

            // Find the shared corner
            if roads.len() > 1 {
                // TODO -1 and not +1 is brittle... must be the angle sorting
                if let Some(l3) =
                    get_outgoing_sidewalk(map, i.id, wraparound_get(&roads, idx1 - 1).0)
                {
                    result.push(Turn {
                        id: turn_id(i.id, l1.id, l3.id),
                        turn_type: TurnType::SharedSidewalkCorner,
                        line: Line::new(l1.last_pt(), l3.first_pt()),
                        lookup_idx: 0,
                    });
                    result.push(Turn {
                        id: turn_id(i.id, l3.id, l1.id),
                        turn_type: TurnType::SharedSidewalkCorner,
                        line: Line::new(l3.first_pt(), l1.last_pt()),
                        lookup_idx: 0,
                    });
                }
            }
        }
    }

    result
}

fn turn_id(parent: IntersectionID, src: LaneID, dst: LaneID) -> TurnID {
    TurnID { parent, src, dst }
}

fn get_incoming_sidewalk(map: &Map, i: IntersectionID, r: RoadID) -> Option<&Lane> {
    get_sidewalk(map, map.get_r(r).incoming_lanes(i))
}

fn get_outgoing_sidewalk(map: &Map, i: IntersectionID, r: RoadID) -> Option<&Lane> {
    get_sidewalk(map, map.get_r(r).outgoing_lanes(i))
}

fn get_sidewalk<'a>(map: &'a Map, children: &Vec<(LaneID, LaneType)>) -> Option<&'a Lane> {
    for (id, lt) in children {
        if *lt == LaneType::Sidewalk {
            return Some(map.get_l(*id));
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

fn make_vehicle_turn(map: &Map, i: IntersectionID, l1: LaneID, l2: LaneID) -> Turn {
    Turn {
        id: turn_id(i, l1, l2),
        turn_type: TurnType::Other,
        line: Line::new(map.get_l(l1).last_pt(), map.get_l(l2).first_pt()),
        lookup_idx: 0,
    }
}

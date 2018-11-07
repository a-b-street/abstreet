use abstutil::{wraparound_get, MultiMap};
use geom::{Angle, Line};
use std::collections::HashSet;
use std::iter;
use {
    Intersection, IntersectionID, Lane, LaneID, LaneType, Map, Road, RoadID, Turn, TurnID, TurnType,
};

pub fn make_all_turns(i: &Intersection, map: &Map) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    turns.extend(make_driving_turns(i, map));
    turns.extend(make_biking_turns(i, map));
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

fn make_driving_turns(i: &Intersection, map: &Map) -> Vec<Turn> {
    if i.is_dead_end(map) {
        return make_driving_turns_for_dead_end(i, map);
    }

    // TODO make get_roads do this?
    let roads: Vec<&Road> = i.get_roads(map).into_iter().map(|r| map.get_r(r)).collect();

    let mut result = Vec::new();

    for r1 in &roads {
        let incoming = filter_driving_lanes(r1.incoming_lanes(i.id));
        if incoming.is_empty() {
            continue;
        }

        for r2 in &roads {
            if r1.id == r2.id {
                continue;
            }
            let outgoing = filter_driving_lanes(r2.outgoing_lanes(i.id));
            if outgoing.is_empty() {
                continue;
            }

            // Use an arbitrary lane from each road to get the angle between r1 and r2.
            let angle1 = map.get_l(incoming[0]).last_line().angle();
            let angle2 = map.get_l(outgoing[0]).first_line().angle();
            let diff = angle1
                .shortest_rotation_towards(angle2)
                .normalized_degrees();

            if diff < 10.0 || diff > 350.0 {
                // Straight. Match up based on the relative number of lanes.
                result.extend(match_up_driving_lanes(map, i.id, &incoming, &outgoing));
            } else if diff > 180.0 {
                // Clockwise rotation means a right turn?
                result.push(make_driving_turn(
                    map,
                    i.id,
                    *incoming.last().unwrap(),
                    *outgoing.last().unwrap(),
                ));
            } else {
                // Counter-clockwise rotation means a left turn
                result.push(make_driving_turn(map, i.id, incoming[0], outgoing[0]));
            }
        }
    }

    result
}

fn match_up_driving_lanes(
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
            result.push(make_driving_turn(map, i, **l1, *l2));
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
            result.push(make_driving_turn(map, i, *l1, **l2));
        }
    } else {
        // The easy case!
        for (l1, l2) in incoming.iter().zip(outgoing.iter()) {
            result.push(make_driving_turn(map, i, *l1, *l2));
        }
    }
    result
}

fn make_driving_turns_for_dead_end(i: &Intersection, map: &Map) -> Vec<Turn> {
    let road = map.get_r(i.get_roads(map).into_iter().next().unwrap());
    let incoming = filter_driving_lanes(road.incoming_lanes(i.id));
    let outgoing = filter_driving_lanes(road.outgoing_lanes(i.id));
    if incoming.is_empty() || outgoing.is_empty() {
        error!("{} needs to be a border node!", i.id);
        return Vec::new();
    }

    match_up_driving_lanes(map, i.id, &incoming, &outgoing)
}

fn make_biking_turns(i: &Intersection, m: &Map) -> Vec<Turn> {
    // TODO Road should make this easier, but how?
    let mut incoming_roads: HashSet<RoadID> = HashSet::new();
    let mut incoming_bike_lanes_per_road: MultiMap<RoadID, LaneID> = MultiMap::new();
    let mut incoming_driving_lanes_per_road: MultiMap<RoadID, LaneID> = MultiMap::new();
    for id in &i.incoming_lanes {
        let l = m.get_l(*id);
        incoming_roads.insert(l.parent);
        match l.lane_type {
            LaneType::Biking => incoming_bike_lanes_per_road.insert(l.parent, *id),
            LaneType::Driving => incoming_driving_lanes_per_road.insert(l.parent, *id),
            _ => {}
        };
    }

    let mut outgoing_roads: HashSet<RoadID> = HashSet::new();
    let mut outgoing_bike_lanes_per_road: MultiMap<RoadID, LaneID> = MultiMap::new();
    let mut outgoing_driving_lanes_per_road: MultiMap<RoadID, LaneID> = MultiMap::new();
    for id in &i.outgoing_lanes {
        let l = m.get_l(*id);
        outgoing_roads.insert(l.parent);
        match l.lane_type {
            LaneType::Biking => outgoing_bike_lanes_per_road.insert(l.parent, *id),
            LaneType::Driving => outgoing_driving_lanes_per_road.insert(l.parent, *id),
            _ => {}
        };
    }

    let mut incoming: Vec<LaneID> = Vec::new();
    for incoming_road in &incoming_roads {
        // Prefer a bike lane if it's there, otherwise use all driving lanes
        let lanes = incoming_bike_lanes_per_road.get(*incoming_road);
        if !lanes.is_empty() {
            incoming.extend(lanes);
        } else {
            incoming.extend(incoming_driving_lanes_per_road.get(*incoming_road));
        }
    }

    let mut outgoing: Vec<LaneID> = Vec::new();
    for outgoing_road in &outgoing_roads {
        let lanes = outgoing_bike_lanes_per_road.get(*outgoing_road);
        if !lanes.is_empty() {
            outgoing.extend(lanes);
        } else {
            outgoing.extend(outgoing_driving_lanes_per_road.get(*outgoing_road));
        }
    }

    // Stay deterministic! Iteration earlier used HashSets.
    incoming.sort();
    outgoing.sort();

    // Kind of a hack. We wind up making some driving->driving turns here, but make_driving_turns
    // will create those, and duplicates are bad. Filter them out here.
    make_turns(m, i.id, &incoming, &outgoing)
        .into_iter()
        .filter(|t| m.get_l(t.id.src).is_biking() || m.get_l(t.id.dst).is_biking())
        .collect()
}

fn make_turns(
    map: &Map,
    parent: IntersectionID,
    incoming: &Vec<LaneID>,
    outgoing: &Vec<LaneID>,
) -> Vec<Turn> {
    // TODO: Figure out why this happens in the huge map
    if incoming.is_empty() {
        if false {
            warn!("{} has no incoming lanes of some type", parent);
        }
        return Vec::new();
    }
    if outgoing.is_empty() {
        if false {
            warn!("{} has no outgoing lanes of some type", parent);
        }
        return Vec::new();
    }

    // Sanity check...
    for l in incoming {
        assert_eq!(map.get_l(*l).dst_i, parent);
    }
    for l in outgoing {
        assert_eq!(map.get_l(*l).src_i, parent);
    }

    let dead_end = map.get_i(parent).is_dead_end(map);

    let mut result = Vec::new();
    for src in incoming {
        let src_l = map.get_l(*src);

        for dst in outgoing {
            let dst_l = map.get_l(*dst);
            // Don't create U-turns unless it's a dead-end
            if src_l.parent == dst_l.parent && !dead_end {
                continue;
            }
            // TODO if it's a multi-lane dead-end, ideally match up lanes or something

            result.push(Turn {
                id: turn_id(parent, src_l.id, dst_l.id),
                turn_type: TurnType::Other,
                line: Line::new(src_l.last_pt(), dst_l.first_pt()),
            });
        }
    }
    result
}

fn make_walking_turns(i: &Intersection, map: &Map) -> Vec<Turn> {
    // Sort roads by the angle into the intersection, so we can reason about sidewalks of adjacent
    // roads.
    let mut roads: Vec<(RoadID, Angle)> = i
        .get_roads(map)
        .into_iter()
        .map(|id| {
            let r = map.get_r(id);

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
                });
                result.push(Turn {
                    id: turn_id(i.id, l2.id, l1.id),
                    turn_type: TurnType::Crosswalk,
                    line: Line::new(l2.first_pt(), l1.last_pt()),
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
                    });
                    result.push(Turn {
                        id: turn_id(i.id, l3.id, l1.id),
                        turn_type: TurnType::SharedSidewalkCorner,
                        line: Line::new(l3.first_pt(), l1.last_pt()),
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

fn filter_driving_lanes(lanes: &Vec<(LaneID, LaneType)>) -> Vec<LaneID> {
    lanes
        .iter()
        .filter_map(|(id, lt)| {
            if *lt == LaneType::Driving {
                Some(*id)
            } else {
                None
            }
        }).collect()
}

fn make_driving_turn(map: &Map, i: IntersectionID, l1: LaneID, l2: LaneID) -> Turn {
    Turn {
        id: turn_id(i, l1, l2),
        turn_type: TurnType::Other,
        line: Line::new(map.get_l(l1).last_pt(), map.get_l(l2).first_pt()),
    }
}

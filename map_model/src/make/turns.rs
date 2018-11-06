use abstutil::{wraparound_get, MultiMap};
use geom::{Angle, Line};
use std::collections::HashSet;
use {Intersection, IntersectionID, Lane, LaneID, LaneType, Map, RoadID, Turn, TurnID, TurnType};

pub fn make_all_turns(i: &Intersection, m: &Map) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    turns.extend(make_driving_turns(i, m));
    turns.extend(make_biking_turns(i, m));
    turns.extend(make_crosswalks(i, m));
    dedupe(turns)
}

fn dedupe(turns: Vec<Turn>) -> Vec<Turn> {
    let mut ids = HashSet::new();
    let mut keep: Vec<Turn> = Vec::new();
    for t in turns.into_iter() {
        if ids.contains(&t.id) {
            // TODO Disable panic so large.abst works :(
            error!("Duplicate turns {}!", t.id);
        } else {
            ids.insert(t.id);
            keep.push(t);
        }
    }
    keep
}

fn make_driving_turns(i: &Intersection, m: &Map) -> Vec<Turn> {
    let incoming: Vec<LaneID> = i.incoming_lanes
        .iter()
        // TODO why's this double borrow happen?
        .filter(|id| m.get_l(**id).is_driving())
        .map(|id| *id)
        .collect();
    let outgoing: Vec<LaneID> = i
        .outgoing_lanes
        .iter()
        .filter(|id| m.get_l(**id).is_driving())
        .map(|id| *id)
        .collect();

    make_turns(m, i.id, &incoming, &outgoing)
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
        .filter(|t| m.get_l(t.src).is_biking() || m.get_l(t.dst).is_biking())
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

            result.push(make_turn(parent, TurnType::Other, src_l, dst_l));
        }
    }
    result
}

fn make_crosswalks(i: &Intersection, map: &Map) -> Vec<Turn> {
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

    if roads.len() < 3 {
        // TODO not yet...
        return Vec::new();
    }

    let mut result: Vec<Turn> = Vec::new();

    // TODO and the mirror ones
    for idx1 in 0..roads.len() as isize {
        if let Some(l1) = get_incoming_sidewalk(map, i.id, wraparound_get(&roads, idx1).0) {
            // TODO -1 and not +1 is brittle... must be the angle sorting
            if let Some(l2) = get_outgoing_sidewalk(map, i.id, wraparound_get(&roads, idx1 - 1).0) {
                let angle_diff = (l1.last_line().angle().normalized_degrees()
                    - l2.first_line().angle().normalized_degrees()).abs();
                // TODO tuning
                if angle_diff < 30.0 {
                    result.push(make_turn(i.id, TurnType::Crosswalk, l1, l2));
                } else {
                    result.push(make_turn(i.id, TurnType::SharedSidewalkCorner, l1, l2));
                    if let Some(l3) =
                        get_outgoing_sidewalk(map, i.id, wraparound_get(&roads, idx1 - 2).0)
                    {
                        let angle_diff = (l1.last_line().angle().normalized_degrees()
                            - l3.first_line().angle().normalized_degrees()).abs();
                        // TODO tuning
                        if angle_diff < 15.0 {
                            result.push(make_turn(i.id, TurnType::Crosswalk, l1, l3));
                        }
                    }
                }
            }
        }
    }

    result
}

fn make_turn(parent: IntersectionID, turn_type: TurnType, src: &Lane, dst: &Lane) -> Turn {
    Turn {
        id: turn_id(parent, src.id, dst.id),
        parent,
        src: src.id,
        dst: dst.id,
        // TODO Won't work for the contraflow cases
        line: Line::new(src.last_pt(), dst.first_pt()),
        turn_type,
    }
}

fn turn_id(parent: IntersectionID, src: LaneID, dst: LaneID) -> TurnID {
    TurnID { parent, src, dst }
}

fn get_incoming_sidewalk(map: &Map, i: IntersectionID, r: RoadID) -> Option<&Lane> {
    let r = map.get_r(r);
    if r.src_i == i {
        get_sidewalk(map, &r.children_backwards)
    } else {
        get_sidewalk(map, &r.children_forwards)
    }
}

fn get_outgoing_sidewalk(map: &Map, i: IntersectionID, r: RoadID) -> Option<&Lane> {
    let r = map.get_r(r);
    if r.src_i == i {
        get_sidewalk(map, &r.children_forwards)
    } else {
        get_sidewalk(map, &r.children_backwards)
    }
}

fn get_sidewalk<'a>(map: &'a Map, children: &Vec<(LaneID, LaneType)>) -> Option<&'a Lane> {
    for (id, lt) in children {
        if *lt == LaneType::Sidewalk {
            return Some(map.get_l(*id));
        }
    }
    None
}

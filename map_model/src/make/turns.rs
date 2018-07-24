use abstutil::MultiMap;
use geom::Line;
use std::collections::HashSet;
use {Intersection, IntersectionID, LaneID, LaneType, Map, RoadID, Turn, TurnID};

pub(crate) fn make_all_turns(i: &Intersection, m: &Map, turn_id_start: usize) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    turns.extend(make_driving_turns(i, m, turn_id_start));
    let len = turns.len();
    turns.extend(make_biking_turns(i, m, turn_id_start + len));
    let len = turns.len();
    turns.extend(make_crosswalks(i, m, turn_id_start + len));
    turns
}

fn make_driving_turns(i: &Intersection, m: &Map, turn_id_start: usize) -> Vec<Turn> {
    let incoming: Vec<LaneID> = i.incoming_lanes
        .iter()
        // TODO why's this double borrow happen?
        .filter(|id| m.get_l(**id).lane_type == LaneType::Driving)
        .map(|id| *id)
        .collect();
    let outgoing: Vec<LaneID> = i.outgoing_lanes
        .iter()
        .filter(|id| m.get_l(**id).lane_type == LaneType::Driving)
        .map(|id| *id)
        .collect();

    make_turns(m, turn_id_start, i.id, &incoming, &outgoing)
}

fn make_biking_turns(i: &Intersection, m: &Map, turn_id_start: usize) -> Vec<Turn> {
    // TODO great further evidence of needing a road/lane distinction
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

    make_turns(m, turn_id_start, i.id, &incoming, &outgoing)
}

fn make_turns(
    map: &Map,
    turn_id_start: usize,
    parent: IntersectionID,
    incoming: &Vec<LaneID>,
    outgoing: &Vec<LaneID>,
) -> Vec<Turn> {
    // TODO: Figure out why this happens in the huge map
    if incoming.is_empty() {
        println!("WARNING: {} has no incoming lanes of some type", parent);
        return Vec::new();
    }
    if outgoing.is_empty() {
        println!("WARNING: {} has no outgoing lanes of some type", parent);
        return Vec::new();
    }
    let dead_end = incoming.len() == 1 && outgoing.len() == 1;

    let mut result = Vec::new();
    for src in incoming {
        let src_l = map.get_l(*src);
        let other_side = map.get_r(src_l.parent).get_opposite_lane(src_l.id, map);

        for dst in outgoing {
            let dst_l = map.get_l(*dst);
            // Don't create U-turns unless it's a dead-end
            if other_side == Some(dst_l.id) && !dead_end {
                continue;
            }

            let id = TurnID(turn_id_start + result.len());
            result.push(Turn {
                id,
                parent,
                src: *src,
                dst: *dst,
                line: Line::new(src_l.last_pt(), dst_l.first_pt()),
                between_sidewalks: false,
            });
        }
    }
    result
}

fn make_crosswalks(i: &Intersection, m: &Map, mut turn_id_start: usize) -> Vec<Turn> {
    let mut result = Vec::new();

    // TODO dedupe some of this logic render/intersection

    // First make all of the crosswalks -- from each incoming and outgoing sidewalk to its other
    // side
    for id in i.incoming_lanes.iter().chain(i.outgoing_lanes.iter()) {
        let src = m.get_l(*id);
        if src.lane_type != LaneType::Sidewalk {
            continue;
        }
        let dst = m.get_l(m.get_r(src.parent).get_opposite_lane(src.id, m).unwrap());

        let id = TurnID(turn_id_start);
        turn_id_start += 1;
        result.push(Turn {
            id,
            parent: i.id,
            src: src.id,
            dst: dst.id,
            line: Line::new(src.endpoint(i.id), dst.endpoint(i.id)),
            between_sidewalks: true,
        });
    }

    // Then all of the immediate connections onto the shared point
    for id1 in i.incoming_lanes.iter().chain(i.outgoing_lanes.iter()) {
        let src = m.get_l(*id1);
        if src.lane_type != LaneType::Sidewalk {
            continue;
        }
        let src_pt = src.endpoint(i.id);
        for id2 in i.incoming_lanes.iter().chain(i.outgoing_lanes.iter()) {
            if id1 == id2 {
                continue;
            }

            let dst = m.get_l(*id2);
            if dst.lane_type != LaneType::Sidewalk {
                continue;
            }
            let dst_pt = dst.endpoint(i.id);

            if src_pt != dst_pt {
                continue;
            }

            let id = TurnID(turn_id_start);
            turn_id_start += 1;
            result.push(Turn {
                id,
                parent: i.id,
                src: src.id,
                dst: dst.id,
                line: Line::new(src_pt, dst_pt),
                between_sidewalks: true,
            });
        }
    }

    result
}

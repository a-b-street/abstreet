use abstutil::MultiMap;
use geom::Line;
use std::collections::HashSet;
use {Intersection, IntersectionID, LaneID, LaneType, Map, RoadID, Turn, TurnID};

pub(crate) fn make_all_turns(i: &Intersection, m: &Map) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    turns.extend(make_driving_turns(i, m));
    turns.extend(make_biking_turns(i, m));
    turns.extend(make_crosswalks(i, m));
    check_dupes(&turns);
    turns
}

fn check_dupes(turns: &Vec<Turn>) {
    let mut ids = HashSet::new();
    for t in turns {
        if ids.contains(&t.id) {
            panic!("Duplicate turns! {:?}", turns);
        }
        ids.insert(t.id);
    }
}

fn make_driving_turns(i: &Intersection, m: &Map) -> Vec<Turn> {
    let incoming: Vec<LaneID> = i.incoming_lanes
        .iter()
        // TODO why's this double borrow happen?
        .filter(|id| m.get_l(**id).is_driving())
        .map(|id| *id)
        .collect();
    let outgoing: Vec<LaneID> = i.outgoing_lanes
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
        println!("WARNING: {} has no incoming lanes of some type", parent);
        return Vec::new();
    }
    if outgoing.is_empty() {
        println!("WARNING: {} has no outgoing lanes of some type", parent);
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
                id: turn_id(parent, *src, *dst),
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

fn make_crosswalks(i: &Intersection, m: &Map) -> Vec<Turn> {
    let mut result = Vec::new();

    // TODO dedupe some of this logic render/intersection

    // First make all of the crosswalks -- from each incoming and outgoing sidewalk to its other
    // side
    for id in i.incoming_lanes.iter().chain(i.outgoing_lanes.iter()) {
        let src = m.get_l(*id);
        if src.lane_type != LaneType::Sidewalk {
            continue;
        }
        let dst = m.get_l(
            m.get_r(src.parent)
                .get_opposite_lane(src.id, LaneType::Sidewalk)
                .unwrap(),
        );

        result.push(Turn {
            id: turn_id(i.id, src.id, dst.id),
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

            result.push(Turn {
                id: turn_id(i.id, src.id, dst.id),
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

fn turn_id(parent: IntersectionID, src: LaneID, dst: LaneID) -> TurnID {
    TurnID { parent, src, dst }
}

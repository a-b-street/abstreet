use geom::Line;
use {Intersection, LaneType, Map, RoadID, Turn, TurnID};

pub(crate) fn make_turns(i: &Intersection, m: &Map, turn_id_start: usize) -> Vec<Turn> {
    let incoming: Vec<RoadID> = i.incoming_roads
        .iter()
        // TODO why's this double borrow happen?
        .filter(|id| m.get_r(**id).lane_type == LaneType::Driving)
        .map(|id| *id)
        .collect();
    let outgoing: Vec<RoadID> = i.outgoing_roads
        .iter()
        .filter(|id| m.get_r(**id).lane_type == LaneType::Driving)
        .map(|id| *id)
        .collect();

    // TODO: Figure out why this happens in the huge map
    if incoming.is_empty() {
        println!("WARNING: intersection {:?} has no incoming roads", i);
        return Vec::new();
    }
    if outgoing.is_empty() {
        println!("WARNING: intersection {:?} has no outgoing roads", i);
        return Vec::new();
    }
    let dead_end = incoming.len() == 1 && outgoing.len() == 1;

    let mut result = Vec::new();
    for src in &incoming {
        let src_r = m.get_r(*src);
        for dst in &outgoing {
            let dst_r = m.get_r(*dst);
            // Don't create U-turns unless it's a dead-end
            if src_r.other_side == Some(dst_r.id) && !dead_end {
                continue;
            }

            let id = TurnID(turn_id_start + result.len());
            result.push(Turn {
                id,
                parent: i.id,
                src: *src,
                dst: *dst,
                line: Line::new(src_r.last_pt(), dst_r.first_pt()),
                between_sidewalks: false,
            });
        }
    }
    result
}

pub(crate) fn make_crosswalks(i: &Intersection, m: &Map, mut turn_id_start: usize) -> Vec<Turn> {
    let mut result = Vec::new();

    // TODO dedupe some of this logic render/intersection

    // First make all of the crosswalks -- from each incoming and outgoing sidewalk to its other
    // side
    for id in i.incoming_roads.iter().chain(i.outgoing_roads.iter()) {
        let src = m.get_r(*id);
        if src.lane_type != LaneType::Sidewalk {
            continue;
        }
        let dst = m.get_r(src.other_side.unwrap());

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
    for id1 in i.incoming_roads.iter().chain(i.outgoing_roads.iter()) {
        let src = m.get_r(*id1);
        if src.lane_type != LaneType::Sidewalk {
            continue;
        }
        let src_pt = src.endpoint(i.id);
        for id2 in i.incoming_roads.iter().chain(i.outgoing_roads.iter()) {
            if id1 == id2 {
                continue;
            }

            let dst = m.get_r(*id2);
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

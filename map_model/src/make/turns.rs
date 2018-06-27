use Map;
use intersection::Intersection;
use road::{LaneType, RoadID};
use turn::{Turn, TurnID};

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
                src_pt: src_r.last_pt(),
                dst_pt: dst_r.first_pt(),
            });
        }
    }
    result
}

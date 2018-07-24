use dimensioned::si;
use geom::PolyLine;
use intersection::Intersection;
use lane::{Lane, LaneID};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub(crate) fn trim_lines(lanes: &mut Vec<Lane>, i: &Intersection) {
    // We update the entire polyline, not the first/last line. A polyline could be trimmed twice --
    // once for each intersection it touches. Since the trimming should only affect one endpoint of
    // the polyline, it's fine to do these separately and in any order.
    let mut shortest_polyline: HashMap<LaneID, (PolyLine, si::Meter<f64>)> = HashMap::new();

    fn update_shortest(
        m: &mut HashMap<LaneID, (PolyLine, si::Meter<f64>)>,
        l: LaneID,
        pl: PolyLine,
    ) {
        let new_len = pl.length();

        match m.entry(l) {
            Entry::Occupied(mut o) => {
                if new_len < o.get().1 {
                    o.insert((pl, new_len));
                }
            }
            Entry::Vacant(v) => {
                v.insert((pl, new_len));
            }
        }
    }

    // This matches by polyline, so short first/last lines should be fine
    for incoming in &i.incoming_lanes {
        for outgoing in &i.outgoing_lanes {
            let pl1 = &lanes[incoming.0].lane_center_pts;
            let pl2 = &lanes[outgoing.0].lane_center_pts;
            if let Some(hit) = pl1.intersection(&pl2) {
                let mut new_pl1 = pl1.clone();
                new_pl1.trim_to_pt(hit);
                update_shortest(&mut shortest_polyline, *incoming, new_pl1);

                let mut new_pl2 = pl2.clone().reversed();
                new_pl2.trim_to_pt(hit);
                update_shortest(&mut shortest_polyline, *outgoing, new_pl2.reversed());
            }
        }
    }

    // Apply the updates
    for (id, pair) in &shortest_polyline {
        lanes[id.0].lane_center_pts = pair.0.clone();
    }
}

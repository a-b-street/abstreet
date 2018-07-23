use dimensioned::si;
use geom::PolyLine;
use intersection::Intersection;
use road::{Road, RoadID};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub(crate) fn trim_lines(roads: &mut Vec<Road>, i: &Intersection) {
    let mut shortest_first_polyline: HashMap<RoadID, (PolyLine, si::Meter<f64>)> = HashMap::new();
    let mut shortest_last_polyline: HashMap<RoadID, (PolyLine, si::Meter<f64>)> = HashMap::new();

    fn update_shortest(m: &mut HashMap<RoadID, (PolyLine, si::Meter<f64>)>, r: RoadID, pl: PolyLine) {
        let new_len = pl.length();

        match m.entry(r) {
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
    for incoming in &i.incoming_roads {
        for outgoing in &i.outgoing_roads {
            let pl1 = &roads[incoming.0].lane_center_pts;
            let pl2 = &roads[outgoing.0].lane_center_pts;
            if let Some(hit) = pl1.intersection(&pl2) {
                let mut new_pl1 = pl1.clone();
                new_pl1.trim_to_pt(hit);
                update_shortest(&mut shortest_last_polyline, *incoming,  new_pl1);

                let mut new_pl2 = pl2.clone().reversed();
                new_pl2.trim_to_pt(hit);
                update_shortest(
                    &mut shortest_first_polyline,
                    *outgoing,
                    new_pl2.reversed(),
                );
            }
        }
    }

    // Apply the updates
    // TODO ah, do we kinda need to merge the two updates? hmm... shortest last will just kinda
    // win. but we do this per intersection, so it should actually be fine! just simplify this.
    for (id, pair) in &shortest_first_polyline {
        roads[id.0]
            .lane_center_pts = pair.0.clone();
    }
    for (id, pair) in &shortest_last_polyline {
        roads[id.0]
            .lane_center_pts = pair.0.clone();
    }
}

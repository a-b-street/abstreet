use Pt2D;
use dimensioned::si;
use geometry;
use intersection::Intersection;
use road::{Road, RoadID};
use std::collections::HashMap;
use std::collections::hash_map::Entry;

pub(crate) fn trim_lines(roads: &mut Vec<Road>, i: &Intersection) {
    let mut shortest_first_line: HashMap<RoadID, (Pt2D, Pt2D, si::Meter<f64>)> = HashMap::new();
    let mut shortest_last_line: HashMap<RoadID, (Pt2D, Pt2D, si::Meter<f64>)> = HashMap::new();

    fn update_shortest(
        m: &mut HashMap<RoadID, (Pt2D, Pt2D, si::Meter<f64>)>,
        r: RoadID,
        l: (Pt2D, Pt2D),
    ) {
        let new_len = geometry::euclid_dist(l);

        match m.entry(r) {
            Entry::Occupied(mut o) => {
                if new_len < o.get().2 {
                    o.insert((l.0, l.1, new_len));
                }
            }
            Entry::Vacant(v) => {
                v.insert((l.0, l.1, new_len));
            }
        }
    }

    // For short first/last lines, this might not work well
    for incoming in &i.incoming_roads {
        for outgoing in &i.outgoing_roads {
            let l1 = roads[incoming.0].last_line();
            let l2 = roads[outgoing.0].first_line();
            if let Some(hit) = geometry::line_segment_intersection(l1, l2) {
                update_shortest(&mut shortest_last_line, *incoming, (l1.0, hit));
                update_shortest(&mut shortest_first_line, *outgoing, (hit, l2.1));
            }
        }
    }

    // Apply the updates
    for (id, triple) in &shortest_first_line {
        roads[id.0].lane_center_pts[0] = triple.0;
        roads[id.0].lane_center_pts[1] = triple.1;
    }
    for (id, triple) in &shortest_last_line {
        let len = roads[id.0].lane_center_pts.len();
        roads[id.0].lane_center_pts[len - 2] = triple.0;
        roads[id.0].lane_center_pts[len - 1] = triple.1;
    }
}

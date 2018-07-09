use dimensioned::si;
use geom::Line;
use intersection::Intersection;
use road::{Road, RoadID};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub(crate) fn trim_lines(roads: &mut Vec<Road>, i: &Intersection) {
    let mut shortest_first_line: HashMap<RoadID, (Line, si::Meter<f64>)> = HashMap::new();
    let mut shortest_last_line: HashMap<RoadID, (Line, si::Meter<f64>)> = HashMap::new();

    fn update_shortest(m: &mut HashMap<RoadID, (Line, si::Meter<f64>)>, r: RoadID, l: Line) {
        let new_len = l.length();

        match m.entry(r) {
            Entry::Occupied(mut o) => {
                if new_len < o.get().1 {
                    o.insert((l, new_len));
                }
            }
            Entry::Vacant(v) => {
                v.insert((l, new_len));
            }
        }
    }

    // For short first/last lines, this might not work well
    for incoming in &i.incoming_roads {
        for outgoing in &i.outgoing_roads {
            let l1 = roads[incoming.0].last_line();
            let l2 = roads[outgoing.0].first_line();
            if let Some(hit) = l1.intersection(&l2) {
                update_shortest(&mut shortest_last_line, *incoming, Line::new(l1.pt1(), hit));
                update_shortest(
                    &mut shortest_first_line,
                    *outgoing,
                    Line::new(hit, l2.pt2()),
                );
            }
        }
    }

    // Apply the updates
    for (id, pair) in &shortest_first_line {
        roads[id.0]
            .lane_center_pts
            .replace_first_line(pair.0.pt1(), pair.0.pt2());
    }
    for (id, pair) in &shortest_last_line {
        roads[id.0]
            .lane_center_pts
            .replace_last_line(pair.0.pt1(), pair.0.pt2());
    }
}

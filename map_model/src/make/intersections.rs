use dimensioned::si;
use geom::{PolyLine, Pt2D};
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use {Road, RoadID, LANE_THICKNESS};

pub fn intersection_polygon(pt: Pt2D, road_ids: BTreeSet<RoadID>, roads: &Vec<Road>) -> Vec<Pt2D> {
    // Turn each incident road into two PolyLines, forming the border of the entire road.
    let mut lines: Vec<PolyLine> = Vec::new();
    for id in road_ids.into_iter() {
        let r = &roads[id.0];
        let fwd_width = LANE_THICKNESS * (r.children_forwards.len() as f64);
        let back_width = LANE_THICKNESS * (r.children_backwards.len() as f64);

        // All of the lines are "incoming" to the intersection pt, meaning their last point is at
        // the intersection.
        let line = &r.center_pts;
        // TODO shift(...).unwrap() should maybe fall back to shift_blindly or something
        if line.first_pt() == pt {
            lines.push(line.shift(fwd_width).unwrap().reversed());
            lines.push(line.reversed().shift(back_width).unwrap());
        } else if line.last_pt() == pt {
            lines.push(line.shift(fwd_width).unwrap());
            lines.push(line.reversed().shift(back_width).unwrap().reversed());
        } else {
            panic!("Incident road {} doesn't have an endpoint at {}", id, pt);
        }
    }

    // Now trim all of the lines against all others.
    // TODO The next step is to just consider adjacent (in the angle sense) pairs and handle ~180
    // deg offsets between pairs.
    // usize indexes into lines
    let mut shortest_line: BTreeMap<usize, (PolyLine, si::Meter<f64>)> = BTreeMap::new();

    fn update_shortest(
        m: &mut BTreeMap<usize, (PolyLine, si::Meter<f64>)>,
        idx: usize,
        pl: PolyLine,
    ) {
        let new_len = pl.length();

        match m.entry(idx) {
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
    for idx1 in 0..lines.len() {
        for idx2 in 0..lines.len() {
            if idx1 == idx2 {
                continue;
            }

            let pl1 = &lines[idx1];
            let pl2 = &lines[idx2];
            if pl1 == pl2 {
                panic!("Both {} and {} have same pts?! {}", idx1, idx2, pl1);
            }

            if let Some(hit) = pl1.intersection(&pl2) {
                let mut new_pl1 = pl1.clone();
                if new_pl1.trim_to_pt(hit) {
                    update_shortest(&mut shortest_line, idx1, new_pl1);
                }

                let mut new_pl2 = pl2.clone();
                if new_pl2.trim_to_pt(hit) {
                    update_shortest(&mut shortest_line, idx2, new_pl2);
                }
            }
        }
    }

    // Apply the updates
    for (idx, (pl, _)) in shortest_line.into_iter() {
        lines[idx] = pl;
    }

    // Now finally use all of the endpoints of the trimmed lines to make a polygon!
    let mut endpoints: Vec<Pt2D> = lines.into_iter().map(|l| l.last_pt()).collect();
    // TODO Safer to not use original intersection pt?
    let center = Pt2D::center(&endpoints);
    // Sort points by angle from the center
    endpoints.sort_by_key(|pt| center.angle_to(*pt).normalized_degrees() as i64);
    // Both lines get trimmed to the same endpoint, so we wind up with dupe points.
    // TODO This entire algorithm could be later simplified
    endpoints.dedup();
    let first_pt = endpoints[0].clone();
    endpoints.push(first_pt);
    endpoints
}

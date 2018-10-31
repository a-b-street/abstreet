use geom::{PolyLine, Pt2D};
use std::collections::BTreeSet;
use {Intersection, Road, RoadID, LANE_THICKNESS};

pub fn intersection_polygon(
    i: &Intersection,
    road_ids: BTreeSet<RoadID>,
    roads: &Vec<Road>,
) -> Vec<Pt2D> {
    // Turn all of the incident roads into the center PolyLine, always pointing at the intersection
    // (endpoint is pt). The f64's are the width to shift without transforming the points, and then
    // the width to shift when reversing the points.
    let mut center_lines: Vec<(PolyLine, RoadID, f64, f64)> = road_ids
        .into_iter()
        .map(|id| {
            let r = &roads[id.0];
            let line = &r.center_pts;
            let fwd_width = LANE_THICKNESS * (r.children_forwards.len() as f64);
            let back_width = LANE_THICKNESS * (r.children_backwards.len() as f64);

            if line.first_pt() == i.point {
                (line.reversed(), id, back_width, fwd_width)
            } else if line.last_pt() == i.point {
                (line.clone(), id, fwd_width, back_width)
            } else {
                panic!("Incident road {} doesn't have an endpoint at {}", id, i.id);
            }
        }).collect();

    // Sort the polylines by the angle of their last segment.
    // TODO This might break weirdly for polylines with very short last lines!
    center_lines.sort_by_key(|(pl, _, _, _)| pl.last_line().angle().normalized_degrees() as i64);

    // Now look at adjacent pairs of these polylines...
    let mut endpoints: Vec<Pt2D> = Vec::new();
    for idx1 in 0..center_lines.len() as isize {
        let idx2 = idx1 + 1;

        let (center1, id1, _, width1_reverse) = wraparound_get(&center_lines, idx1);
        let (center2, id2, width2_normal, _) = wraparound_get(&center_lines, idx2);

        // Turn the center polylines into one of the road's border polylines. Every road should
        // have a chance to be shifted in both directions.
        let pl1 = center1
            .reversed()
            .shift(*width1_reverse)
            .unwrap()
            .reversed();
        let pl2 = center2.shift(*width2_normal).unwrap();

        if let Some(hit) = pl1.intersection(&pl2) {
            endpoints.push(hit);
        } else {
            warn!(
                "No hit btwn {} and {}, for {} with {} incident roads",
                id1,
                id2,
                i.id,
                center_lines.len()
            );
            endpoints.push(pl1.last_pt());
            endpoints.push(pl2.last_pt());
        }
    }

    // Close off the polygon
    let first_pt = endpoints[0].clone();
    endpoints.push(first_pt);
    endpoints
}

fn wraparound_get<T>(vec: &Vec<T>, idx: isize) -> &T {
    let len = vec.len() as isize;
    let idx = idx % len;
    let idx = if idx >= 0 { idx } else { idx + len };
    &vec[idx as usize]
}

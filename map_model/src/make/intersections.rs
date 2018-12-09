use crate::{Intersection, Road, RoadID, LANE_THICKNESS};
use abstutil::wraparound_get;
use dimensioned::si;
use geom::{Angle, PolyLine, Pt2D};
use std::marker;

const DEGENERATE_INTERSECTION_HALF_LENGTH: si::Meter<f64> = si::Meter {
    value_unsafe: 5.0,
    _marker: marker::PhantomData,
};

// The polygon should exist entirely within the thick bands around all original roads -- it just
// carves up part of that space, doesn't reach past it.
pub fn intersection_polygon(i: &Intersection, roads: &Vec<Road>) -> Vec<Pt2D> {
    // Turn all of the incident roads into two PolyLines (the "forwards" and "backwards" borders of
    // the road), both with an endpoint at i.point, and the angle of the last segment of the center
    // line.
    let mut lines: Vec<(RoadID, Angle, PolyLine, PolyLine)> = i
        .roads
        .iter()
        .map(|id| {
            let r = &roads[id.0];
            let fwd_width = LANE_THICKNESS * (r.children_forwards.len() as f64);
            let back_width = LANE_THICKNESS * (r.children_backwards.len() as f64);

            let (line, width_normal, width_reverse) = if r.src_i == i.id {
                (r.center_pts.reversed(), back_width, fwd_width)
            } else if r.dst_i == i.id {
                (r.center_pts.clone(), fwd_width, back_width)
            } else {
                panic!("Incident road {} doesn't have an endpoint at {}", id, i.id);
            };

            let pl_normal = line.shift(width_normal).unwrap();
            let pl_reverse = line.reversed().shift(width_reverse).unwrap().reversed();
            (*id, line.last_line().angle(), pl_normal, pl_reverse)
        })
        .collect();

    // Sort the polylines by the angle of their last segment.
    // TODO This might break weirdly for polylines with very short last lines!
    lines.sort_by_key(|(_, angle, _, _)| angle.normalized_degrees() as i64);

    // Special cases for degenerate intersections.
    let mut endpoints: Vec<Pt2D> = Vec::new();
    if lines.len() == 1 {
        // Dead-ends!
        let (id, _, pl_a, pl_b) = &lines[0];
        let pt1 = pl_a
            .reversed()
            .safe_dist_along(DEGENERATE_INTERSECTION_HALF_LENGTH * 2.0)
            .map(|(pt, _)| pt);
        let pt2 = pl_b
            .reversed()
            .safe_dist_along(DEGENERATE_INTERSECTION_HALF_LENGTH * 2.0)
            .map(|(pt, _)| pt);
        if pt1.is_some() && pt2.is_some() {
            endpoints.extend(vec![
                pt1.unwrap(),
                pt2.unwrap(),
                pl_b.last_pt(),
                pl_a.last_pt(),
            ]);
        } else {
            error!("{} is a dead-end for {}, which is too short to make degenerate intersection geometry", i.id, id);
            endpoints.extend(vec![pl_a.last_pt(), pl_b.last_pt()]);
        }
    } else if lines.len() == 2 {
        let (id1, _, pl1_a, pl1_b) = &lines[0];
        let (id2, _, pl2_a, pl2_b) = &lines[1];
        endpoints.extend(
            vec![pl1_a, pl1_b, pl2_a, pl2_b]
                .into_iter()
                .filter_map(|l| {
                    l.reversed()
                        .safe_dist_along(DEGENERATE_INTERSECTION_HALF_LENGTH)
                        .map(|(pt, _)| pt)
                })
                .collect::<Vec<Pt2D>>(),
        );
        if endpoints.len() != 4 {
            error!("{} has only {} and {}, some of which are too short to make degenerate intersection geometry", i.id, id1, id2);
            endpoints.clear();
            endpoints.extend(vec![
                pl1_a.last_pt(),
                pl1_b.last_pt(),
                pl2_a.last_pt(),
                pl2_b.last_pt(),
            ]);
        }
    } else {
        // Look at adjacent pairs of these polylines...
        for idx1 in 0..lines.len() as isize {
            let idx2 = idx1 + 1;

            let (id1, _, _, pl1) = wraparound_get(&lines, idx1);
            let (id2, _, pl2, _) = wraparound_get(&lines, idx2);

            // If the two lines are too close in angle, they'll either not hit or even if they do, it
            // won't be right.
            let angle_diff = (pl1.last_line().angle().opposite().normalized_degrees()
                - pl2.last_line().angle().normalized_degrees())
            .abs();

            // TODO A tuning challenge. :)
            if angle_diff > 15.0 {
                // The easy case!
                if let Some(hit) = pl1.intersection(&pl2) {
                    endpoints.push(hit);
                    continue;
                }
            }

            let mut ok = true;

            // Use the next adjacent road, doing line to line segment intersection instead.
            let inf_line1 = wraparound_get(&lines, idx1 - 1).3.last_line();
            if let Some(hit) = pl1.intersection_infinite_line(inf_line1) {
                endpoints.push(hit);
            } else {
                endpoints.push(pl1.last_pt());
                ok = false;
            }

            let inf_line2 = wraparound_get(&lines, idx2 + 1).2.last_line();
            if let Some(hit) = pl2.intersection_infinite_line(inf_line2) {
                endpoints.push(hit);
            } else {
                endpoints.push(pl2.last_pt());
                ok = false;
            }

            if !ok {
                warn!(
                    "No hit btwn {} and {}, for {} with {} incident roads",
                    id1,
                    id2,
                    i.id,
                    lines.len()
                );
            }
        }
    }

    // Close off the polygon
    endpoints.push(endpoints[0]);
    endpoints
}

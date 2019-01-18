use crate::{Intersection, IntersectionID, Road, RoadID, LANE_THICKNESS};
use abstutil::note;
use abstutil::wraparound_get;
use dimensioned::si;
use geom::{Angle, Line, PolyLine, Pt2D};
use std::marker;

const DEGENERATE_INTERSECTION_HALF_LENGTH: si::Meter<f64> = si::Meter {
    value_unsafe: 5.0,
    _marker: marker::PhantomData,
};

// The polygon should exist entirely within the thick bands around all original roads -- it just
// carves up part of that space, doesn't reach past it.
pub fn intersection_polygon(i: &Intersection, roads: &mut Vec<Road>) -> Vec<Pt2D> {
    // Turn all of the incident roads into two PolyLines (the "forwards" and "backwards" borders of
    // the road, if the roads were oriented to both be incoming to the intersection), both ending
    // at the intersection (which may be different points for merged intersections!), and the angle
    // of the last segment of the center line.
    // TODO Maybe express the two incoming PolyLines as the "right" and "left"
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

            let pl_normal = line.shift_right(width_normal);
            let pl_reverse = line.shift_left(width_reverse);
            (*id, line.last_line().angle(), pl_normal, pl_reverse)
        })
        .collect();

    // Sort the polylines by the angle of their last segment.
    // TODO This might break weirdly for polylines with very short last lines!
    // TODO This definitely can break for merged intersections. To get the lines "in order", maybe
    // we have to look at all the endpoints and sort by angle from the center of the points?
    lines.sort_by_key(|(_, angle, _, _)| angle.normalized_degrees() as i64);

    let mut endpoints = if lines.len() == 1 {
        deadend(roads, i.id, &lines)
    } else if lines.len() == 2 {
        degenerate_twoway(roads, i.id, &lines)
    } else if let Some(pts) = make_new_polygon(roads, i.id, &lines) {
        pts
    } else if let Some(pts) = make_thick_thin_threeway(roads, i.id, &lines) {
        pts
    } else {
        note(format!(
            "couldnt make new for {} with {} roads",
            i.id,
            lines.len()
        ));
        make_old_polygon(&lines)
    };

    // Close off the polygon
    endpoints.push(endpoints[0]);
    endpoints
}

fn deadend(
    roads: &mut Vec<Road>,
    i: IntersectionID,
    lines: &Vec<(RoadID, Angle, PolyLine, PolyLine)>,
) -> Vec<Pt2D> {
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
        let mut r = &mut roads[id.0];
        if r.src_i == i {
            r.center_pts = r
                .center_pts
                .slice(
                    DEGENERATE_INTERSECTION_HALF_LENGTH * 2.0,
                    r.center_pts.length(),
                )
                .0;
        } else {
            r.center_pts = r
                .center_pts
                .slice(
                    0.0 * si::M,
                    r.center_pts.length() - DEGENERATE_INTERSECTION_HALF_LENGTH * 2.0,
                )
                .0;
        }

        vec![pt1.unwrap(), pt2.unwrap(), pl_b.last_pt(), pl_a.last_pt()]
    } else {
        error!(
            "{} is a dead-end for {}, which is too short to make degenerate intersection geometry",
            i, id
        );
        vec![pl_a.last_pt(), pl_b.last_pt()]
    }
}

fn degenerate_twoway(
    roads: &mut Vec<Road>,
    i: IntersectionID,
    lines: &Vec<(RoadID, Angle, PolyLine, PolyLine)>,
) -> Vec<Pt2D> {
    let (id1, _, pl1_a, pl1_b) = &lines[0];
    let (id2, _, pl2_a, pl2_b) = &lines[1];

    if roads[id1.0].center_pts.length() >= DEGENERATE_INTERSECTION_HALF_LENGTH
        && roads[id2.0].center_pts.length() >= DEGENERATE_INTERSECTION_HALF_LENGTH
    {
        // Why fix center pts and then re-shift out, instead of use the pl1_a and friends? because
        // dist_along on shifted polylines is NOT equivalent.
        let mut endpoints = Vec::new();
        for road_id in &[id1, id2] {
            let mut r = &mut roads[road_id.0];
            if r.src_i == i {
                r.center_pts = r
                    .center_pts
                    .slice(DEGENERATE_INTERSECTION_HALF_LENGTH, r.center_pts.length())
                    .0;

                endpoints.push(
                    r.center_pts
                        .shift_left(LANE_THICKNESS * (r.children_backwards.len() as f64))
                        .first_pt(),
                );
                endpoints.push(
                    r.center_pts
                        .shift_right(LANE_THICKNESS * (r.children_forwards.len() as f64))
                        .first_pt(),
                );
            } else {
                r.center_pts = r
                    .center_pts
                    .slice(
                        0.0 * si::M,
                        r.center_pts.length() - DEGENERATE_INTERSECTION_HALF_LENGTH,
                    )
                    .0;
                endpoints.push(
                    r.center_pts
                        .shift_right(LANE_THICKNESS * (r.children_forwards.len() as f64))
                        .last_pt(),
                );
                endpoints.push(
                    r.center_pts
                        .shift_left(LANE_THICKNESS * (r.children_backwards.len() as f64))
                        .last_pt(),
                );
            }
        }
        endpoints
    } else {
        error!("{} has only {} and {}, some of which are too short to make degenerate intersection geometry", i, id1, id2);
        vec![
            pl1_a.last_pt(),
            pl1_b.last_pt(),
            pl2_a.last_pt(),
            pl2_b.last_pt(),
        ]
    }
}

fn make_new_polygon(
    roads: &mut Vec<Road>,
    i: IntersectionID,
    lines: &Vec<(RoadID, Angle, PolyLine, PolyLine)>,
) -> Option<Vec<Pt2D>> {
    // Since we might fail halfway through this function, don't actually trim center lines until we
    // know we'll succeed.
    let mut new_road_centers: Vec<(RoadID, PolyLine)> = Vec::new();

    let mut endpoints: Vec<Pt2D> = Vec::new();
    // Find the two corners of each road
    for idx in 0..lines.len() as isize {
        let (id, _, fwd_pl, back_pl) = wraparound_get(&lines, idx);
        let (_back_id, _, adj_back_pl, _) = wraparound_get(&lines, idx + 1);
        let (_fwd_id, _, _, adj_fwd_pl) = wraparound_get(&lines, idx - 1);

        // road_center ends at the intersection.
        // TODO This is redoing some work. :\
        let road_center = if roads[id.0].dst_i == i {
            roads[id.0].center_pts.clone()
        } else {
            roads[id.0].center_pts.reversed()
        };

        // If the adjacent polylines don't intersect at all, then we have something like a
        // three-way intersection (or maybe just a case where the angles of the two adjacent roads
        // are super close). In that case, we only have one corner to choose as a candidate for
        // trimming back the road center.
        let (fwd_hit, new_center1) = {
            if let Some((hit, angle)) = fwd_pl.intersection(adj_fwd_pl) {
                // Find where the perpendicular to this corner hits the original line
                let perp = Line::new(hit, hit.project_away(1.0, angle.rotate_degs(90.0)));
                let trim_to = road_center.intersection_infinite_line(perp)?;
                (Some(hit), Some(road_center.trim_to_pt(trim_to)))
            } else {
                (None, None)
            }
        };
        let (back_hit, new_center2) = {
            if let Some((hit, angle)) = back_pl.intersection(adj_back_pl) {
                // Find where the perpendicular to this corner hits the original line
                let perp = Line::new(hit, hit.project_away(1.0, angle.rotate_degs(90.0)));
                let trim_to = road_center.intersection_infinite_line(perp)?;
                (Some(hit), Some(road_center.trim_to_pt(trim_to)))
            } else {
                (None, None)
            }
        };

        let shorter_center = match (new_center1, new_center2) {
            (Some(c1), Some(c2)) => {
                if c1.length() <= c2.length() {
                    c1
                } else {
                    c2
                }
            }
            (Some(c1), None) => c1,
            (None, Some(c2)) => c2,
            (None, None) => {
                // TODO We might need to revert some shortened road centers!
                return None;
            }
        };

        // TODO This is redoing LOTS of work
        let r = &mut roads[id.0];
        let fwd_width = LANE_THICKNESS * (r.children_forwards.len() as f64);
        let back_width = LANE_THICKNESS * (r.children_backwards.len() as f64);

        let (width_normal, width_reverse) = if r.src_i == i {
            new_road_centers.push((*id, shorter_center.reversed()));
            (back_width, fwd_width)
        } else {
            new_road_centers.push((*id, shorter_center.clone()));
            (fwd_width, back_width)
        };
        let pl_normal = shorter_center.shift_right(width_normal);
        let pl_reverse = shorter_center.shift_left(width_reverse);

        // Toss in the original corners, so the intersection polygon doesn't cover area not
        // originally covered by the thick road bands.
        if let Some(hit) = fwd_hit {
            endpoints.push(hit);
        }
        endpoints.push(pl_normal.last_pt());
        endpoints.push(pl_reverse.last_pt());
        if let Some(hit) = back_hit {
            endpoints.push(hit);
        }
    }

    for (id, pl) in new_road_centers {
        roads[id.0].center_pts = pl;
    }

    Some(approx_dedupe(endpoints))
}

fn make_old_polygon(lines: &Vec<(RoadID, Angle, PolyLine, PolyLine)>) -> Vec<Pt2D> {
    let mut endpoints = Vec::new();
    // Look at adjacent pairs of these polylines...
    for idx1 in 0..lines.len() as isize {
        let idx2 = idx1 + 1;

        let (_, _, _, pl1) = wraparound_get(&lines, idx1);
        let (_, _, pl2, _) = wraparound_get(&lines, idx2);

        // If the two lines are too close in angle, they'll either not hit or even if they do, it
        // won't be right.
        let angle_diff = (pl1.last_line().angle().opposite().normalized_degrees()
            - pl2.last_line().angle().normalized_degrees())
        .abs();

        // TODO A tuning challenge. :)
        if angle_diff > 15.0 {
            // The easy case!
            if let Some((hit, _)) = pl1.intersection(&pl2) {
                endpoints.push(hit);
                continue;
            }
        }

        // Use the next adjacent road, doing line to line segment intersection instead.
        let inf_line1 = wraparound_get(&lines, idx1 - 1).3.last_line();
        if let Some(hit) = pl1.intersection_infinite_line(inf_line1) {
            endpoints.push(hit);
        } else {
            endpoints.push(pl1.last_pt());
        }

        let inf_line2 = wraparound_get(&lines, idx2 + 1).2.last_line();
        if let Some(hit) = pl2.intersection_infinite_line(inf_line2) {
            endpoints.push(hit);
        } else {
            endpoints.push(pl2.last_pt());
        }
    }
    endpoints
}

// Temporary until Pt2D has proper resolution.
fn approx_dedupe(pts: Vec<Pt2D>) -> Vec<Pt2D> {
    let mut result: Vec<Pt2D> = Vec::new();
    for pt in pts {
        if result.is_empty() || !result.last().unwrap().approx_eq(pt) {
            result.push(pt);
        }
    }
    result
}

// Does the _a or _b line of any of the roads completely cross another road? This happens often
// when normal roads intersect a highway on/off ramp, or more generally, when the width of one road
// is very different than the others.
fn make_thick_thin_threeway(
    roads: &mut Vec<Road>,
    i: IntersectionID,
    lines: &Vec<(RoadID, Angle, PolyLine, PolyLine)>,
) -> Option<Vec<Pt2D>> {
    if lines.len() != 3 {
        return None;
    }

    for thick_idx in 0..3 {
        for thick_side in &[true, false] {
            let (thick_id, thick_pl) = if *thick_side {
                let (id, _, _, pl) = &lines[thick_idx];
                (id, pl)
            } else {
                let (id, _, pl, _) = &lines[thick_idx];
                (id, pl)
            };

            for thin_idx in 0..3 {
                if thin_idx == thick_idx {
                    continue;
                }
                let (thin_id, _, thin_a, thin_b) = &lines[thin_idx];
                if thick_pl.intersection(&thin_a).is_none()
                    || thick_pl.intersection(&thin_b).is_none()
                {
                    continue;
                }

                let thin_pl = if *thick_side { thin_a } else { thin_b };

                let (thick_pt1, thick_pt2) =
                    trim_to_hit(&mut roads[thick_id.0], i, thick_pl, thin_pl);
                let (thin_pt1, thin_pt2) = trim_to_hit(&mut roads[thin_id.0], i, thin_pl, thick_pl);

                // Leave the other line alone.
                let (_, _, other_a, other_b) = &lines[other_idx(thick_idx, thin_idx)];

                if *thick_side {
                    return Some(vec![
                        thick_pt1,
                        thick_pt2,
                        thin_pt1,
                        thin_pt2,
                        other_a.last_pt(),
                        other_b.last_pt(),
                    ]);
                } else {
                    return Some(vec![
                        thick_pt1,
                        thick_pt2,
                        other_a.last_pt(),
                        other_b.last_pt(),
                        thin_pt1,
                        thin_pt2,
                    ]);
                }
            }
        }
    }

    None
}

// These are helpers for make_thick_thin_threeway.

// Returns the two endpoints for the intersection polygon after trimming, in the (forwards,
// backwards) order.
fn trim_to_hit(
    r: &mut Road,
    i: IntersectionID,
    our_pl: &PolyLine,
    other_pl: &PolyLine,
) -> (Pt2D, Pt2D) {
    // Find the spot along the road's original center that's perpendicular to the hit. Keep in
    // mind our_pl might not be the road's center.
    let orig_center = if r.dst_i == i {
        r.center_pts.clone()
    } else {
        r.center_pts.reversed()
    };

    let (hit, angle) = our_pl.intersection(other_pl).unwrap();
    let perp = Line::new(hit, hit.project_away(1.0, angle.rotate_degs(90.0)));
    let trim_to = orig_center.intersection_infinite_line(perp).unwrap();
    let new_center = orig_center.trim_to_pt(trim_to);

    // TODO Really redoing work. :\
    let fwd_width = LANE_THICKNESS * (r.children_forwards.len() as f64);
    let back_width = LANE_THICKNESS * (r.children_backwards.len() as f64);

    if r.dst_i == i {
        r.center_pts = new_center;

        (
            r.center_pts.shift_right(fwd_width).last_pt(),
            r.center_pts.shift_left(back_width).last_pt(),
        )
    } else {
        r.center_pts = new_center.reversed();

        (
            r.center_pts.shift_left(back_width).first_pt(),
            r.center_pts.shift_right(fwd_width).first_pt(),
        )
    }
}

fn other_idx(idx1: usize, idx2: usize) -> usize {
    if idx1 != 0 && idx2 != 0 {
        return 0;
    }
    if idx1 != 1 && idx2 != 1 {
        return 1;
    }
    2
}

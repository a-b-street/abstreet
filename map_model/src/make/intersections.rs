use crate::{Intersection, IntersectionID, Road, RoadID, LANE_THICKNESS};
//use abstutil::note;
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
    } else {
        /*note(format!(
            "couldnt make new for {} with {} roads",
            i.id,
            lines.len()
        ));*/
        make_old_polygon(i.id, &lines)
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
                // TODO This fails for something in the 23rd Ave map.
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
                // TODO This doesn't work yet, and it's getting VERY complicated.
                /*
                // Different strategy. Take the perpendicular infinite line and intersect with the
                // adjacent line that does NOT share an endpoint.
                let fwd_same_endpt = fwd_pl.last_pt() == adj_fwd_pl.last_pt();
                let back_same_endpt = back_pl.last_pt() == adj_back_pl.last_pt();

                let debug = i.0 == 357;
                if debug {
                    note(format!(
                        "{} adjacent to {} fwd, {} back. same endpts: {} and {}",
                        id, fwd_id, back_id, fwd_same_endpt, back_same_endpt
                    ));
                }

                if (fwd_same_endpt || back_same_endpt) && !(fwd_same_endpt && back_same_endpt) {
                    if fwd_same_endpt {
                        let perp = Line::new(back_pl.last_pt(), back_pl.last_pt().project_away(1.0, back_pl.last_line().angle().rotate_degs(90.0)));
                        let adj_hit = adj_back_pl.intersection_infinite_line(perp)?;
                        endpoints.push(fwd_pl.last_pt());
                        endpoints.push(adj_hit);
                    } else {
                        let perp = Line::new(fwd_pl.last_pt(), fwd_pl.last_pt().project_away(1.0, fwd_pl.last_line().angle().rotate_degs(90.0)));
                        let adj_hit = adj_fwd_pl.intersection_infinite_line(perp)?;
                        endpoints.push(adj_hit);
                        endpoints.push(back_pl.last_pt());
                    }
                    continue;
                } else {
                    // TODO whoa, how's this happen?
                    return None;
                }
                */
                return None;
            }
        };

        // TODO This is redoing LOTS of work
        let r = &mut roads[id.0];
        let fwd_width = LANE_THICKNESS * (r.children_forwards.len() as f64);
        let back_width = LANE_THICKNESS * (r.children_backwards.len() as f64);

        let (width_normal, width_reverse) = if r.src_i == i {
            r.center_pts = shorter_center.reversed();
            (back_width, fwd_width)
        } else {
            r.center_pts = shorter_center.clone();
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

    Some(approx_dedupe(endpoints))
}

fn make_old_polygon(
    i: IntersectionID,
    lines: &Vec<(RoadID, Angle, PolyLine, PolyLine)>,
) -> Vec<Pt2D> {
    let mut endpoints = Vec::new();
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
            if let Some((hit, _)) = pl1.intersection(&pl2) {
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
                i,
                lines.len()
            );
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

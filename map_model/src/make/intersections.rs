use crate::{Intersection, IntersectionID, Road, RoadID, LANE_THICKNESS};
use abstutil::wraparound_get;
use dimensioned::si;
use geom::{Angle, HashablePt2D, Line, PolyLine, Pt2D};
use std::collections::HashMap;
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
    } else {
        generalized_trim_back(roads, i.id, &lines)
    };

    // Close off the polygon
    endpoints.push(endpoints[0]);
    endpoints
}

fn generalized_trim_back(
    roads: &mut Vec<Road>,
    i: IntersectionID,
    lines: &Vec<(RoadID, Angle, PolyLine, PolyLine)>,
) -> Vec<Pt2D> {
    let mut road_lines: Vec<(RoadID, &PolyLine)> = Vec::new();
    for (r, _, pl1, pl2) in lines {
        road_lines.push((*r, pl1));
        road_lines.push((*r, pl2));
    }

    let mut new_road_centers: HashMap<RoadID, PolyLine> = HashMap::new();

    // Intersect every road's boundary lines with all the other lines
    for (r1, pl1) in &road_lines {
        // road_center ends at the intersection.
        let road_center = if roads[r1.0].dst_i == i {
            roads[r1.0].center_pts.clone()
        } else {
            roads[r1.0].center_pts.reversed()
        };

        // Always trim back a minimum amount, if possible.
        let mut shortest_center = if road_center.length() >= DEGENERATE_INTERSECTION_HALF_LENGTH {
            road_center
                .slice(
                    0.0 * si::M,
                    road_center.length() - DEGENERATE_INTERSECTION_HALF_LENGTH,
                )
                .0
        } else {
            road_center.clone()
        };

        for (r2, pl2) in &road_lines {
            if r1 == r2 {
                continue;
            }

            if let Some((hit, angle)) = pl1.intersection(pl2) {
                // Find where the perpendicular hits the original road line
                let perp = Line::new(hit, hit.project_away(1.0, angle.rotate_degs(90.0)));
                // How could something perpendicular to a shifted polyline never hit the original
                // polyline?
                let trim_to = road_center.intersection_infinite_line(perp).unwrap();
                let trimmed = road_center.trim_to_pt(trim_to);
                if trimmed.length() < shortest_center.length() {
                    shortest_center = trimmed;
                }

                // We could also do the update for r2, but we'll just get to it later.
            }
        }

        let new_center = if roads[r1.0].dst_i == i {
            shortest_center
        } else {
            shortest_center.reversed()
        };
        if let Some(existing) = new_road_centers.get(r1) {
            if new_center.length() < existing.length() {
                new_road_centers.insert(*r1, new_center);
            }
        } else {
            new_road_centers.insert(*r1, new_center);
        }
    }

    // After doing all the intersection checks, copy over the new centers. Also shift those centers
    // out again to find the endpoints that'll make up the polygon.
    let mut endpoints: Vec<Pt2D> = Vec::new();
    for (id, center_pts) in new_road_centers {
        let mut r = &mut roads[id.0];
        r.center_pts = center_pts;

        let fwd_width = LANE_THICKNESS * (r.children_forwards.len() as f64);
        let back_width = LANE_THICKNESS * (r.children_backwards.len() as f64);

        if r.dst_i == i {
            endpoints.push(r.center_pts.shift_right(fwd_width).last_pt());
            endpoints.push(r.center_pts.shift_left(back_width).last_pt());
        } else {
            endpoints.push(r.center_pts.shift_right(fwd_width).first_pt());
            endpoints.push(r.center_pts.shift_left(back_width).first_pt());
        }
    }
    // Include collisions between polylines of adjacent roads, so the polygon doesn't cover area
    // not originally covered by the thick road bands.
    for idx in 0..lines.len() as isize {
        let (_, _, fwd_pl, back_pl) = wraparound_get(&lines, idx);
        let (_, _, adj_back_pl, _) = wraparound_get(&lines, idx + 1);
        let (_, _, _, adj_fwd_pl) = wraparound_get(&lines, idx - 1);

        if let Some((hit, _)) = fwd_pl.intersection(adj_fwd_pl) {
            endpoints.push(hit);
        }
        if let Some((hit, _)) = back_pl.intersection(adj_back_pl) {
            endpoints.push(hit);
        }
    }
    endpoints.sort_by_key(|pt| HashablePt2D::from(*pt));
    endpoints = approx_dedupe(endpoints);

    let center = Pt2D::center(&endpoints);
    endpoints.sort_by_key(|pt| Line::new(center, *pt).angle().normalized_degrees() as i64);
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

// Temporary until Pt2D has proper resolution.
fn approx_dedupe(pts: Vec<Pt2D>) -> Vec<Pt2D> {
    let mut result: Vec<Pt2D> = Vec::new();
    for pt in pts {
        if result.is_empty() || !result.last().unwrap().approx_eq(pt, 1.0 * si::M) {
            result.push(pt);
        }
    }
    result
}

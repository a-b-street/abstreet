use crate::make::initial::{Intersection, Road};
use crate::raw_data::{StableIntersectionID, StableRoadID};
use abstutil::wraparound_get;
use geom::{Angle, Distance, HashablePt2D, Line, PolyLine, Pt2D};
use std::collections::{BTreeMap, HashMap};

const DEGENERATE_INTERSECTION_HALF_LENGTH: Distance = Distance::const_meters(5.0);

// The polygon should exist entirely within the thick bands around all original roads -- it just
// carves up part of that space, doesn't reach past it.
pub fn intersection_polygon(
    i: &Intersection,
    roads: &mut BTreeMap<StableRoadID, Road>,
) -> Vec<Pt2D> {
    // Turn all of the incident roads into two PolyLines (the "forwards" and "backwards" borders of
    // the road, if the roads were oriented to both be incoming to the intersection), both ending
    // at the intersection (which may be different points for merged intersections!), and the angle
    // of the last segment of the center line.
    // TODO Maybe express the two incoming PolyLines as the "right" and "left"
    let mut lines: Vec<(StableRoadID, Angle, PolyLine, PolyLine)> = i
        .roads
        .iter()
        .map(|id| {
            let r = &roads[id];

            let (line, width_normal, width_reverse) = if r.src_i == i.id {
                (r.trimmed_center_pts.reversed(), r.back_width, r.fwd_width)
            } else if r.dst_i == i.id {
                (r.trimmed_center_pts.clone(), r.fwd_width, r.back_width)
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
    roads: &mut BTreeMap<StableRoadID, Road>,
    i: StableIntersectionID,
    lines: &Vec<(StableRoadID, Angle, PolyLine, PolyLine)>,
) -> Vec<Pt2D> {
    let mut road_lines: Vec<(StableRoadID, PolyLine)> = Vec::new();
    for (r, _, pl1, pl2) in lines {
        road_lines.push((*r, pl1.clone()));
        road_lines.push((*r, pl2.clone()));
    }

    // TODO We can overwrite trimmed now
    let mut new_road_centers: HashMap<StableRoadID, PolyLine> = HashMap::new();

    // Intersect every road's boundary lines with all the other lines
    for (r1, pl1) in &road_lines {
        // road_center ends at the intersection.
        let road_center = if roads[r1].dst_i == i {
            roads[r1].trimmed_center_pts.clone()
        } else {
            roads[r1].trimmed_center_pts.reversed()
        };

        // Always trim back a minimum amount, if possible.
        let mut shortest_center = if road_center.length() >= DEGENERATE_INTERSECTION_HALF_LENGTH {
            road_center
                .slice(
                    Distance::ZERO,
                    road_center.length() - DEGENERATE_INTERSECTION_HALF_LENGTH,
                )
                .unwrap()
                .0
        } else {
            road_center.clone()
        };

        for (r2, pl2) in &road_lines {
            if r1 == r2 {
                continue;
            }

            // If two roads go between the same intersections, they'll likely hit at the wrong
            // side. Just use the second half of the polyline to circumvent this. But sadly, doing
            // this in general breaks other cases -- sometimes we want to find the collision
            // farther away from the intersection in question.
            let same_endpoints = {
                let ii1 = roads[r1].src_i;
                let ii2 = roads[r1].dst_i;
                let ii3 = roads[r2].src_i;
                let ii4 = roads[r2].dst_i;
                (ii1 == ii3 && ii2 == ii4) || (ii1 == ii4 && ii2 == ii3)
            };
            let (use_pl1, use_pl2): (PolyLine, PolyLine) = if same_endpoints {
                (pl1.second_half(), pl2.second_half())
            } else {
                (pl1.clone(), pl2.clone())
            };

            if let Some((hit, angle)) = use_pl1.intersection(&use_pl2) {
                // Find where the perpendicular hits the original road line
                let perp =
                    Line::new(hit, hit.project_away(1.0, angle.rotate_degs(90.0))).infinite();
                // How could something perpendicular to a shifted polyline never hit the original
                // polyline?
                let trim_to = road_center.intersection_infinite(&perp).unwrap();
                let trimmed = road_center.get_slice_ending_at(trim_to).unwrap();
                if trimmed.length() < shortest_center.length() {
                    shortest_center = trimmed;
                }

                // We could also do the update for r2, but we'll just get to it later.
            }
        }

        let new_center = if roads[r1].dst_i == i {
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
        let r = roads.get_mut(&id).unwrap();
        r.trimmed_center_pts = center_pts;

        if r.dst_i == i {
            endpoints.push(r.trimmed_center_pts.shift_right(r.fwd_width).last_pt());
            endpoints.push(r.trimmed_center_pts.shift_left(r.back_width).last_pt());
        } else {
            endpoints.push(r.trimmed_center_pts.shift_right(r.fwd_width).first_pt());
            endpoints.push(r.trimmed_center_pts.shift_left(r.back_width).first_pt());
        }
    }
    // Include collisions between polylines of adjacent roads, so the polygon doesn't cover area
    // not originally covered by the thick road bands.
    for idx in 0..lines.len() as isize {
        let (_, _, fwd_pl, back_pl) = wraparound_get(&lines, idx);
        let (_, _, adj_back_pl, _) = wraparound_get(&lines, idx + 1);
        let (_, _, _, adj_fwd_pl) = wraparound_get(&lines, idx - 1);

        // Hmm, seems to be safe to always take the second half here, but not earlier in general.
        if let Some((hit, _)) = fwd_pl.second_half().intersection(&adj_fwd_pl.second_half()) {
            endpoints.push(hit);
        }
        if let Some((hit, _)) = back_pl
            .second_half()
            .intersection(&adj_back_pl.second_half())
        {
            endpoints.push(hit);
        }
    }
    endpoints.sort_by_key(|pt| HashablePt2D::from(*pt));
    endpoints = Pt2D::approx_dedupe(endpoints, Distance::meters(1.0));

    let center = Pt2D::center(&endpoints);
    endpoints.sort_by_key(|pt| Line::new(center, *pt).angle().normalized_degrees() as i64);
    endpoints
}

fn deadend(
    roads: &mut BTreeMap<StableRoadID, Road>,
    i: StableIntersectionID,
    lines: &Vec<(StableRoadID, Angle, PolyLine, PolyLine)>,
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
        let r = roads.get_mut(&id).unwrap();
        if r.src_i == i {
            r.trimmed_center_pts = r
                .trimmed_center_pts
                .slice(
                    DEGENERATE_INTERSECTION_HALF_LENGTH * 2.0,
                    r.trimmed_center_pts.length(),
                )
                .unwrap()
                .0;
        } else {
            r.trimmed_center_pts = r
                .trimmed_center_pts
                .slice(
                    Distance::ZERO,
                    r.trimmed_center_pts.length() - DEGENERATE_INTERSECTION_HALF_LENGTH * 2.0,
                )
                .unwrap()
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

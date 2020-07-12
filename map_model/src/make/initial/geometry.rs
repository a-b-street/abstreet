use crate::make::initial::{Intersection, Road};
use crate::raw::{DrivingSide, OriginalIntersection, OriginalRoad};
use abstutil::{wraparound_get, Timer};
use geom::{Distance, Line, PolyLine, Polygon, Pt2D};
use std::collections::BTreeMap;

const DEGENERATE_INTERSECTION_HALF_LENGTH: Distance = Distance::const_meters(2.5);

// The polygon should exist entirely within the thick bands around all original roads -- it just
// carves up part of that space, doesn't reach past it.
// Also returns a list of labeled polygons for debugging.
pub fn intersection_polygon(
    driving_side: DrivingSide,
    i: &Intersection,
    roads: &mut BTreeMap<OriginalRoad, Road>,
    timer: &mut Timer,
) -> (Vec<Pt2D>, Vec<(String, Polygon)>) {
    if i.roads.is_empty() {
        panic!("{} has no roads", i.id);
    }
    let mut road_endpts: Vec<Pt2D> = Vec::new();

    // Turn all of the incident roads into two PolyLines (the "forwards" and "backwards" borders of
    // the road, if the roads were oriented to both be incoming to the intersection), both ending
    // at the intersection (which may be different points for merged intersections!), and the last
    // segment of the center line.
    // TODO Maybe express the two incoming PolyLines as the "right" and "left"
    let mut lines: Vec<(OriginalRoad, Line, PolyLine, PolyLine)> = i
        .roads
        .iter()
        .map(|id| {
            let r = &roads[id];

            let pl = if r.src_i == i.id {
                r.trimmed_center_pts.reversed()
            } else if r.dst_i == i.id {
                r.trimmed_center_pts.clone()
            } else {
                panic!("Incident road {} doesn't have an endpoint at {}", id, i.id);
            };
            road_endpts.push(pl.last_pt());

            let pl_normal = driving_side.right_shift(pl.clone(), r.half_width);
            let pl_reverse = driving_side.left_shift(pl.clone(), r.half_width);
            (*id, pl.last_line(), pl_normal, pl_reverse)
        })
        .collect();

    // Find the average of all road endpoints at the intersection. This is usually just a single
    // point, except for merged intersections.
    road_endpts.sort_by_key(|pt| pt.to_hashable());
    road_endpts.dedup();
    let intersection_center = Pt2D::center(&road_endpts);

    // Sort the polylines by the angle their last segment makes to the "center". This is normally
    // equivalent to the angle of the last line, except when the intersection has been merged.
    lines.sort_by_key(|(_, l, _, _)| {
        l.pt1().angle_to(intersection_center).normalized_degrees() as i64
    });

    if lines.len() == 1 {
        deadend(driving_side, roads, i.id, &lines, timer)
    } else {
        generalized_trim_back(driving_side, roads, i.id, &lines, timer)
    }
}

fn generalized_trim_back(
    driving_side: DrivingSide,
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: OriginalIntersection,
    lines: &Vec<(OriginalRoad, Line, PolyLine, PolyLine)>,
    timer: &mut Timer,
) -> (Vec<Pt2D>, Vec<(String, Polygon)>) {
    let mut debug = Vec::new();

    let mut road_lines: Vec<(OriginalRoad, PolyLine, PolyLine)> = Vec::new();
    for (r, _, pl1, pl2) in lines {
        // TODO Argh, just use original lines.
        road_lines.push((*r, pl1.clone(), pl2.clone()));
        road_lines.push((*r, pl2.clone(), pl1.clone()));

        if false {
            debug.push((
                format!("{} fwd", r.osm_way_id),
                pl1.make_polygons(Distance::meters(1.0)),
            ));
            debug.push((
                format!("{} back", r.osm_way_id),
                pl2.make_polygons(Distance::meters(1.0)),
            ));
        }
    }

    let mut new_road_centers: BTreeMap<OriginalRoad, PolyLine> = BTreeMap::new();

    // Intersect every road's boundary lines with all the other lines. Only side effect here is to
    // populate new_road_centers.
    for (r1, pl1, other_pl1) in &road_lines {
        // road_center ends at the intersection.
        let road_center = if roads[r1].dst_i == i {
            roads[r1].trimmed_center_pts.clone()
        } else {
            roads[r1].trimmed_center_pts.reversed()
        };

        // Always trim back a minimum amount, if possible.
        let mut shortest_center = if road_center.length()
            >= DEGENERATE_INTERSECTION_HALF_LENGTH + 3.0 * geom::EPSILON_DIST
        {
            road_center.exact_slice(
                Distance::ZERO,
                road_center.length() - DEGENERATE_INTERSECTION_HALF_LENGTH,
            )
        } else {
            road_center.clone()
        };

        for (r2, pl2, _) in &road_lines {
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

            // TODO This only happens in Austin so far, haven't dove into why.
            if use_pl1 == use_pl2 {
                timer.warn(format!("{} and {} wind up with the same polyline", r1, r2));
                continue;
            }

            if let Some((hit, angle)) = use_pl1.intersection(&use_pl2) {
                // Find where the perpendicular hits the original road line
                let perp = Line::must_new(
                    hit,
                    hit.project_away(Distance::meters(1.0), angle.rotate_degs(90.0)),
                )
                .infinite();
                // How could something perpendicular to a shifted polyline never hit the original
                // polyline? Also, find the hit closest to the intersection -- this matters for
                // very curvy roads, like highway ramps.
                if let Some(trimmed) = road_center
                    .reversed()
                    .intersection_infinite(&perp)
                    .and_then(|trim_to| road_center.get_slice_ending_at(trim_to))
                {
                    if trimmed.length() < shortest_center.length() {
                        shortest_center = trimmed;
                    }
                } else {
                    timer.warn(format!(
                        "{} and {} hit, but the perpendicular never hit the original center line, \
                         or the trimmed thing is empty",
                        r1, r2
                    ));
                }

                // We could also do the update for r2, but we'll just get to it later.
            }

            // Another check... sometimes a boundary line crosss the perpendicular end of another
            // road.
            // TODO Reduce DEGENERATE_INTERSECTION_HALF_LENGTH to play with this.
            if false {
                let perp = Line::must_new(pl1.last_pt(), other_pl1.last_pt());
                if perp.intersection(&pl2.last_line()).is_some() {
                    let new_perp = Line::must_new(
                        pl2.last_pt(),
                        pl2.last_pt()
                            .project_away(Distance::meters(1.0), perp.angle()),
                    )
                    .infinite();
                    // Find the hit closest to the intersection -- this matters for very curvy
                    // roads, like highway ramps.
                    if let Some(trim_to) = road_center.reversed().intersection_infinite(&new_perp) {
                        let trimmed = road_center.get_slice_ending_at(trim_to).unwrap();
                        if trimmed.length() < shortest_center.length() {
                            shortest_center = trimmed;
                        }
                    }
                }
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

    // After doing all the intersection checks, copy over the new centers. Also fill out the
    // intersection polygon's points along the way.
    let mut endpoints: Vec<Pt2D> = Vec::new();
    for idx in 0..lines.len() as isize {
        let (id, _, fwd_pl, back_pl) = wraparound_get(&lines, idx);
        // TODO Ahhh these names are confusing. Adjacent to the fwd_pl, but it's a back pl.
        let (_adj_back_id, _, adj_back_pl, _) = wraparound_get(&lines, idx + 1);
        let (_adj_fwd_id, _, _, adj_fwd_pl) = wraparound_get(&lines, idx - 1);

        roads.get_mut(&id).unwrap().trimmed_center_pts = new_road_centers[&id].clone();
        let r = &roads[&id];

        // Include collisions between polylines of adjacent roads, so the polygon doesn't cover area
        // not originally covered by the thick road bands.
        // It's apparently safe to always take the second_half here.
        if fwd_pl.length() >= geom::EPSILON_DIST * 3.0
            && adj_fwd_pl.length() >= geom::EPSILON_DIST * 3.0
        {
            if let Some((hit, _)) = fwd_pl.second_half().intersection(&adj_fwd_pl.second_half()) {
                endpoints.push(hit);
            } else {
                // Style 1
                /*endpoints.push(fwd_pl.last_pt());
                endpoints.push(adj_fwd_pl.last_pt());*/

                // Style 2
                /*if let Some(hit) = fwd_pl
                    .last_line()
                    .infinite()
                    .intersection(&adj_fwd_pl.last_line().infinite())
                {
                    endpoints.push(hit);
                }*/
            }
        } else {
            timer.warn(format!(
                "Excluding collision between original polylines of {} and something, because \
                 stuff's too short",
                id
            ));
        }

        // Shift those final centers out again to find the main endpoints for the polygon.
        if r.dst_i == i {
            endpoints.push(
                driving_side
                    .right_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .last_pt(),
            );
            endpoints.push(
                driving_side
                    .left_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .last_pt(),
            );
        } else {
            endpoints.push(
                driving_side
                    .left_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .first_pt(),
            );
            endpoints.push(
                driving_side
                    .right_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .first_pt(),
            );
        }

        if back_pl.length() >= geom::EPSILON_DIST * 3.0
            && adj_back_pl.length() >= geom::EPSILON_DIST * 3.0
        {
            if let Some((hit, _)) = back_pl
                .second_half()
                .intersection(&adj_back_pl.second_half())
            {
                endpoints.push(hit);
            } else {
                // Style 1
                /*endpoints.push(back_pl.last_pt());
                endpoints.push(adj_back_pl.last_pt());*/

                // Style 2
                /*if let Some(hit) = back_pl
                    .last_line()
                    .infinite()
                    .intersection(&adj_back_pl.last_line().infinite())
                {
                    endpoints.push(hit);
                }*/
            }
        } else {
            timer.warn(format!(
                "Excluding collision between original polylines of {} and something, because \
                 stuff's too short",
                id
            ));
        }

        /*if *id == OriginalRoad(384) {
            let thin = Distance::meters(1.0);
            debug.push((format!("back of {}", id), back_pl.make_polygons(thin)));
            debug.push((
                format!("adj back of {}", adj_back_id),
                adj_back_pl.make_polygons(thin),
            ));
        }*/
    }

    let main_result = close_off_polygon(Pt2D::approx_dedupe(endpoints, Distance::meters(0.1)));

    // There are bad polygons caused by weird short roads. As a temporary workaround, detect cases
    // where polygons dramatically double back on themselves and force the polygon to proceed
    // around its center.
    let mut deduped = main_result.clone();
    deduped.pop();
    deduped.sort_by_key(|pt| pt.to_hashable());
    deduped = Pt2D::approx_dedupe(deduped, Distance::meters(0.1));
    let center = Pt2D::center(&deduped);
    deduped.sort_by_key(|pt| pt.angle_to(center).normalized_degrees() as i64);
    deduped = Pt2D::approx_dedupe(deduped, Distance::meters(0.1));
    deduped = close_off_polygon(deduped);
    if main_result.len() == deduped.len() {
        (main_result, debug)
    } else {
        timer.warn(format!(
            "{}'s polygon has weird repeats, forcibly removing points",
            i
        ));
        (deduped, debug)
    }

    // TODO Or always sort points?
    /*endpoints.sort_by_key(|pt| pt.to_hashable());
    endpoints = Pt2D::approx_dedupe(endpoints, Distance::meters(0.1));
    let center = Pt2D::center(&endpoints);
    endpoints.sort_by_key(|pt| pt.angle_to(center).normalized_degrees() as i64);
    (close_off_polygon(endpoints), debug)*/
}

fn deadend(
    driving_side: DrivingSide,
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: OriginalIntersection,
    lines: &Vec<(OriginalRoad, Line, PolyLine, PolyLine)>,
    timer: &mut Timer,
) -> (Vec<Pt2D>, Vec<(String, Polygon)>) {
    let len = DEGENERATE_INTERSECTION_HALF_LENGTH * 4.0;

    let (id, _, pl_a, pl_b) = &lines[0];
    if pl_a.length() > len && pl_b.length() > len {
        let r = roads.get_mut(&id).unwrap();
        if r.trimmed_center_pts.length() >= len + 3.0 * geom::EPSILON_DIST {
            if r.src_i == i {
                r.trimmed_center_pts = r
                    .trimmed_center_pts
                    .exact_slice(len, r.trimmed_center_pts.length());
            } else {
                r.trimmed_center_pts = r
                    .trimmed_center_pts
                    .exact_slice(Distance::ZERO, r.trimmed_center_pts.length() - len);
            }

            // After trimming the center points, the two sides of the road may be at different
            // points, so shift the center out again to find the endpoints.
            // TODO Refactor with generalized_trim_back.
            let mut endpts = vec![pl_b.last_pt(), pl_a.last_pt()];
            if r.dst_i == i {
                endpts.push(
                    driving_side
                        .right_shift(r.trimmed_center_pts.clone(), r.half_width)
                        .last_pt(),
                );
                endpts.push(
                    driving_side
                        .left_shift(r.trimmed_center_pts.clone(), r.half_width)
                        .last_pt(),
                );
            } else {
                endpts.push(
                    driving_side
                        .left_shift(r.trimmed_center_pts.clone(), r.half_width)
                        .first_pt(),
                );
                endpts.push(
                    driving_side
                        .right_shift(r.trimmed_center_pts.clone(), r.half_width)
                        .first_pt(),
                );
            }

            return (close_off_polygon(endpts), Vec::new());
        }
    }

    timer.warn(format!(
        "{} is a dead-end for {}, which is too short to make degenerate intersection geometry",
        i, id
    ));
    (
        vec![pl_a.last_pt(), pl_b.last_pt(), pl_a.last_pt()],
        Vec::new(),
    )
}

fn close_off_polygon(mut pts: Vec<Pt2D>) -> Vec<Pt2D> {
    if pts.last().unwrap().approx_eq(pts[0], Distance::meters(0.1)) {
        pts.pop();
    }
    pts.push(pts[0]);
    pts
}

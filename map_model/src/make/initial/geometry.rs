use crate::make::initial::{Intersection, Road};
use crate::osm;
use crate::raw::{DrivingSide, OriginalIntersection, OriginalRoad};
use abstutil::{wraparound_get, Timer};
use geom::{Distance, Line, PolyLine, Polygon, Pt2D, Ring, EPSILON_DIST};
use std::collections::BTreeMap;
use std::error::Error;

const DEGENERATE_INTERSECTION_HALF_LENGTH: Distance = Distance::const_meters(2.5);

// The polygon should exist entirely within the thick bands around all original roads -- it just
// carves up part of that space, doesn't reach past it.
// Also returns a list of labeled polygons for debugging.
pub fn intersection_polygon(
    driving_side: DrivingSide,
    i: &Intersection,
    roads: &mut BTreeMap<OriginalRoad, Road>,
    timer: &mut Timer,
) -> Result<(Polygon, Vec<(String, Polygon)>), Box<dyn Error>> {
    if i.roads.is_empty() {
        panic!("{} has no roads", i.id);
    }

    // Turn all of the incident roads into two PolyLines (the "forwards" and "backwards" borders of
    // the road, if the roads were oriented to both be incoming to the intersection), both ending
    // at the intersection, and the last segment of the center line.
    // TODO Maybe express the two incoming PolyLines as the "right" and "left"
    let mut lines: Vec<(OriginalRoad, Line, PolyLine, PolyLine)> = Vec::new();
    for id in &i.roads {
        let r = &roads[id];

        let pl = if r.src_i == i.id {
            r.trimmed_center_pts.reversed()
        } else if r.dst_i == i.id {
            r.trimmed_center_pts.clone()
        } else {
            panic!("Incident road {} doesn't have an endpoint at {}", id, i.id);
        };
        let pl_normal = driving_side.right_shift(pl.clone(), r.half_width)?;
        let pl_reverse = driving_side.left_shift(pl.clone(), r.half_width)?;
        lines.push((*id, pl.last_line(), pl_normal, pl_reverse));
    }

    // Sort the polylines by the angle their last segment makes to the common point.
    let intersection_center = lines[0].1.pt2();
    lines.sort_by_key(|(_, l, _, _)| {
        l.pt1().angle_to(intersection_center).normalized_degrees() as i64
    });

    if lines.len() == 1 {
        return deadend(driving_side, roads, i.id, &lines);
    }
    let rollback = lines
        .iter()
        .map(|(r, _, _, _)| (*r, roads[r].trimmed_center_pts.clone()))
        .collect::<Vec<_>>();
    if let Some(result) = on_off_ramp(driving_side, roads, i.id, lines.clone()) {
        Ok(result)
    } else {
        for (r, trimmed_center_pts) in rollback {
            roads.get_mut(&r).unwrap().trimmed_center_pts = trimmed_center_pts;
        }
        generalized_trim_back(driving_side, roads, i.id, &lines, timer)
    }
}

fn generalized_trim_back(
    driving_side: DrivingSide,
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: OriginalIntersection,
    lines: &Vec<(OriginalRoad, Line, PolyLine, PolyLine)>,
    timer: &mut Timer,
) -> Result<(Polygon, Vec<(String, Polygon)>), Box<dyn Error>> {
    let mut debug = Vec::new();

    let mut road_lines: Vec<(OriginalRoad, PolyLine)> = Vec::new();
    for (r, _, pl1, pl2) in lines {
        road_lines.push((*r, pl1.clone()));
        road_lines.push((*r, pl2.clone()));

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

    // Intersect every road's boundary lines with all the other lines. Only side effect here is to
    // populate new_road_centers.
    let mut new_road_centers: BTreeMap<OriginalRoad, PolyLine> = BTreeMap::new();
    for (r1, pl1) in &road_lines {
        // road_center ends at the intersection.
        let road_center = if roads[r1].dst_i == i {
            roads[r1].trimmed_center_pts.clone()
        } else {
            roads[r1].trimmed_center_pts.reversed()
        };

        // Always trim back a minimum amount, if possible.
        let mut shortest_center =
            if road_center.length() >= DEGENERATE_INTERSECTION_HALF_LENGTH + 3.0 * EPSILON_DIST {
                road_center.exact_slice(
                    Distance::ZERO,
                    road_center.length() - DEGENERATE_INTERSECTION_HALF_LENGTH,
                )
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

            if use_pl1 == use_pl2 {
                return Err(format!(
                    "{} and {} have overlapping segments. You likely need to fix OSM and make the \
                     two ways meet at exactly one node.",
                    r1, r2
                )
                .into());
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
        if fwd_pl.length() >= EPSILON_DIST * 3.0 && adj_fwd_pl.length() >= EPSILON_DIST * 3.0 {
            if let Some((hit, _)) = fwd_pl.second_half().intersection(&adj_fwd_pl.second_half()) {
                endpoints.push(hit);
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
                    .right_shift(r.trimmed_center_pts.clone(), r.half_width)?
                    .last_pt(),
            );
            endpoints.push(
                driving_side
                    .left_shift(r.trimmed_center_pts.clone(), r.half_width)?
                    .last_pt(),
            );
        } else {
            endpoints.push(
                driving_side
                    .left_shift(r.trimmed_center_pts.clone(), r.half_width)?
                    .first_pt(),
            );
            endpoints.push(
                driving_side
                    .right_shift(r.trimmed_center_pts.clone(), r.half_width)?
                    .first_pt(),
            );
        }

        if back_pl.length() >= EPSILON_DIST * 3.0 && adj_back_pl.length() >= EPSILON_DIST * 3.0 {
            if let Some((hit, _)) = back_pl
                .second_half()
                .intersection(&adj_back_pl.second_half())
            {
                endpoints.push(hit);
            }
        } else {
            timer.warn(format!(
                "Excluding collision between original polylines of {} and something, because \
                 stuff's too short",
                id
            ));
        }
    }

    // There are bad polygons caused by weird short roads. As a temporary workaround, detect cases
    // where polygons dramatically double back on themselves and force the polygon to proceed
    // around its center.
    let main_result = close_off_polygon(Pt2D::approx_dedupe(endpoints, Distance::meters(0.1)));
    let mut deduped = main_result.clone();
    deduped.pop();
    deduped.sort_by_key(|pt| pt.to_hashable());
    deduped = Pt2D::approx_dedupe(deduped, Distance::meters(0.1));
    let center = Pt2D::center(&deduped);
    deduped.sort_by_key(|pt| pt.angle_to(center).normalized_degrees() as i64);
    deduped = Pt2D::approx_dedupe(deduped, Distance::meters(0.1));
    deduped = close_off_polygon(deduped);
    if main_result.len() == deduped.len() {
        Ok((Ring::must_new(main_result).to_polygon(), debug))
    } else {
        timer.warn(format!(
            "{}'s polygon has weird repeats, forcibly removing points",
            i
        ));
        Ok((Ring::must_new(deduped).to_polygon(), debug))
    }

    // TODO Or always sort points? Helps some cases, hurts other for downtown Seattle.
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
) -> Result<(Polygon, Vec<(String, Polygon)>), Box<dyn Error>> {
    let len = DEGENERATE_INTERSECTION_HALF_LENGTH * 4.0;

    let (id, _, mut pl_a, mut pl_b) = lines[0].clone();
    // If the lines are too short (usually due to the boundary polygon cutting off border roads too
    // much), just extend them.
    // TODO Not sure why we need +1.5x more, but this looks better. Some math is definitely off
    // somewhere.
    pl_a = pl_a.extend_to_length(len + 1.5 * DEGENERATE_INTERSECTION_HALF_LENGTH);
    pl_b = pl_b.extend_to_length(len + 1.5 * DEGENERATE_INTERSECTION_HALF_LENGTH);

    let r = roads.get_mut(&id).unwrap();
    let len_with_buffer = len + 3.0 * EPSILON_DIST;
    let trimmed = if r.trimmed_center_pts.length() >= len_with_buffer {
        if r.src_i == i {
            r.trimmed_center_pts = r
                .trimmed_center_pts
                .exact_slice(len, r.trimmed_center_pts.length());
        } else {
            r.trimmed_center_pts = r
                .trimmed_center_pts
                .exact_slice(Distance::ZERO, r.trimmed_center_pts.length() - len);
        }
        r.trimmed_center_pts.clone()
    } else {
        if r.src_i == i {
            r.trimmed_center_pts.extend_to_length(len_with_buffer)
        } else {
            r.trimmed_center_pts
                .reversed()
                .extend_to_length(len_with_buffer)
                .reversed()
        }
    };

    // After trimming the center points, the two sides of the road may be at different
    // points, so shift the center out again to find the endpoints.
    // TODO Refactor with generalized_trim_back.
    let mut endpts = vec![pl_b.last_pt(), pl_a.last_pt()];
    if r.dst_i == i {
        endpts.push(
            driving_side
                .right_shift(trimmed.clone(), r.half_width)?
                .last_pt(),
        );
        endpts.push(
            driving_side
                .left_shift(trimmed.clone(), r.half_width)?
                .last_pt(),
        );
    } else {
        endpts.push(
            driving_side
                .left_shift(trimmed.clone(), r.half_width)?
                .first_pt(),
        );
        endpts.push(driving_side.right_shift(trimmed, r.half_width)?.first_pt());
    }

    endpts.dedup();
    Ok((
        Ring::must_new(close_off_polygon(endpts)).to_polygon(),
        Vec::new(),
    ))
}

fn close_off_polygon(mut pts: Vec<Pt2D>) -> Vec<Pt2D> {
    if pts.last().unwrap().approx_eq(pts[0], Distance::meters(0.1)) {
        pts.pop();
    }
    pts.push(pts[0]);
    pts
}

// The lines all end at the intersection
struct Piece {
    id: OriginalRoad,
    left: PolyLine,
    center: PolyLine,
    right: PolyLine,
}

// Try to apply to any 3-way. Might fail for many reasons.
fn on_off_ramp(
    driving_side: DrivingSide,
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: OriginalIntersection,
    lines: Vec<(OriginalRoad, Line, PolyLine, PolyLine)>,
) -> Option<(Polygon, Vec<(String, Polygon)>)> {
    if lines.len() != 3 {
        return None;
    }
    // TODO Really this should apply based on some geometric consideration (one of the endpoints
    // totally inside the other thick road's polygon), but for the moment, this is an OK filter.
    //
    // Example candidate: https://www.openstreetmap.org/node/32177767
    let mut ok = false;
    for (r, _, _, _) in &lines {
        if roads[r].osm_tags.is_any(
            osm::HIGHWAY,
            vec![
                "motorway",
                "motorway_link",
                "primary_link",
                "secondary_link",
                "tertiary_link",
                "trunk_link",
            ],
        ) {
            ok = true;
            break;
        }
    }
    if !ok {
        return None;
    }

    let mut debug = Vec::new();

    let mut pieces = Vec::new();
    // TODO Use this abstraction for all the code here?
    for (id, _, right, left) in lines {
        let r = &roads[&id];
        let center = if r.dst_i == i {
            r.trimmed_center_pts.clone()
        } else {
            r.trimmed_center_pts.reversed()
        };
        pieces.push(Piece {
            id,
            left,
            center,
            right,
        });
    }

    pieces.sort_by_key(|r| roads[&r.id].half_width);
    let thick1 = pieces.pop().unwrap();
    let thick2 = pieces.pop().unwrap();
    let thin = pieces.pop().unwrap();

    // Find where the thin hits the thick farthest along.
    // (trimmed thin center, trimmed thick center, the thick road we hit)
    let mut best_hit: Option<(PolyLine, PolyLine, OriginalRoad)> = None;
    for thin_pl in vec![&thin.left, &thin.right] {
        for thick in vec![&thick1, &thick2] {
            for thick_pl in vec![&thick.left, &thick.right] {
                if let Some((hit, angle)) = thin_pl.intersection(thick_pl) {
                    // Find where the perpendicular hits the original road line
                    // TODO Refactor something to go from a hit+angle on a left/right to a trimmed
                    // center.
                    let perp = Line::must_new(
                        hit,
                        hit.project_away(Distance::meters(1.0), angle.rotate_degs(90.0)),
                    )
                    .infinite();
                    let trimmed_thin = thin
                        .center
                        .reversed()
                        .intersection_infinite(&perp)
                        .and_then(|trim_to| thin.center.get_slice_ending_at(trim_to))?;

                    // Do the same for the thick road
                    let (_, angle) = thick_pl.dist_along_of_point(hit).unwrap();
                    let perp = Line::must_new(
                        hit,
                        hit.project_away(Distance::meters(1.0), angle.rotate_degs(90.0)),
                    )
                    .infinite();
                    let trimmed_thick = thick
                        .center
                        .reversed()
                        .intersection_infinite(&perp)
                        .and_then(|trim_to| thick.center.get_slice_ending_at(trim_to))?;

                    if false {
                        debug.push((
                            format!("1"),
                            geom::Circle::new(hit, Distance::meters(3.0)).to_polygon(),
                        ));
                        debug.push((
                            format!("2"),
                            geom::Circle::new(trimmed_thin.last_pt(), Distance::meters(3.0))
                                .to_polygon(),
                        ));
                        debug.push((
                            format!("3"),
                            geom::Circle::new(trimmed_thick.last_pt(), Distance::meters(3.0))
                                .to_polygon(),
                        ));
                    }
                    if best_hit
                        .as_ref()
                        .map(|(pl, _, _)| trimmed_thin.length() < pl.length())
                        .unwrap_or(true)
                    {
                        best_hit = Some((trimmed_thin, trimmed_thick, thick.id));
                    }
                }
            }
        }
    }

    {
        // Trim the thin
        let (mut trimmed_thin, mut trimmed_thick, thick_id) = best_hit?;
        if roads[&thin.id].dst_i != i {
            trimmed_thin = trimmed_thin.reversed();
        }
        roads.get_mut(&thin.id).unwrap().trimmed_center_pts = trimmed_thin;

        // Trim the thick
        // extra ends at the intersection
        let extra = if roads[&thick_id].dst_i == i {
            roads[&thick_id]
                .trimmed_center_pts
                .get_slice_starting_at(trimmed_thick.last_pt())?
        } else {
            trimmed_thick = trimmed_thick.reversed();
            roads[&thick_id]
                .trimmed_center_pts
                .get_slice_ending_at(trimmed_thick.first_pt())?
                .reversed()
        };
        roads.get_mut(&thick_id).unwrap().trimmed_center_pts = trimmed_thick;
        // Give the merge point some length
        if extra.length() <= 2.0 * DEGENERATE_INTERSECTION_HALF_LENGTH + 3.0 * EPSILON_DIST {
            return None;
        }
        let extra = extra.exact_slice(2.0 * DEGENERATE_INTERSECTION_HALF_LENGTH, extra.length());

        // Now the crazy part -- take the other thick, and LENGTHEN it
        let other = roads
            .get_mut(if thick1.id == thick_id {
                &thick2.id
            } else {
                &thick1.id
            })
            .unwrap();
        if other.dst_i == i {
            other.trimmed_center_pts = other
                .trimmed_center_pts
                .clone()
                .extend(extra.reversed())
                .ok()?;
        } else {
            other.trimmed_center_pts = extra.extend(other.trimmed_center_pts.clone()).ok()?;
        }
    }

    // Now build the actual polygon
    let mut endpoints = Vec::new();
    for id in vec![thin.id, thick1.id, thick2.id] {
        let r = &roads[&id];
        // Shift those final centers out again to find the main endpoints for the polygon.
        if r.dst_i == i {
            endpoints.push(
                driving_side
                    .right_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .ok()?
                    .last_pt(),
            );
            endpoints.push(
                driving_side
                    .left_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .ok()?
                    .last_pt(),
            );
        } else {
            endpoints.push(
                driving_side
                    .left_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .ok()?
                    .first_pt(),
            );
            endpoints.push(
                driving_side
                    .right_shift(r.trimmed_center_pts.clone(), r.half_width)
                    .ok()?
                    .first_pt(),
            );
        }
    }
    endpoints.sort_by_key(|pt| pt.to_hashable());
    endpoints.dedup();
    let center = Pt2D::center(&endpoints);
    endpoints.sort_by_key(|pt| pt.angle_to(center).normalized_degrees() as i64);
    endpoints.dedup();
    Some((
        Ring::must_new(close_off_polygon(endpoints)).to_polygon(),
        debug,
    ))

    //let dummy = geom::Circle::new(orig_lines[0].3.last_pt(), Distance::meters(3.0)).to_polygon();
    //Some((close_off_polygon(dummy.into_points()), debug))
}

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use abstutil::wraparound_get;
use geom::{Circle, Distance, InfiniteLine, Line, PolyLine, Polygon, Pt2D, Ring, EPSILON_DIST};

use crate::initial::Road;
use crate::{osm, OriginalRoad};

const DEGENERATE_INTERSECTION_HALF_LENGTH: Distance = Distance::const_meters(2.5);

// TODO Dedupe with Piece!
#[derive(Clone)]
struct RoadLine {
    id: OriginalRoad,
    sorting_pt: Pt2D,
    center_pl: PolyLine,
    // Both are oriented to be incoming to the intersection (ending at it).
    // TODO Maybe express as the "right" and "left"
    fwd_pl: PolyLine,
    back_pl: PolyLine,
}

/// Also returns a list of labeled polygons for debugging.
///
/// Ideally, the resulting polygon should exist entirely within the thick bands around all original
/// roads -- it just carves up part of that space, doesn't reach past it. But that's not always true
/// yet.
pub fn intersection_polygon(
    intersection_id: osm::NodeID,
    intersection_roads: BTreeSet<OriginalRoad>,
    roads: &mut BTreeMap<OriginalRoad, Road>,
    trim_roads_for_merging: &BTreeMap<(osm::WayID, bool), Pt2D>,
) -> Result<(Polygon, Vec<(String, Polygon)>)> {
    if intersection_roads.is_empty() {
        panic!("{} has no roads", intersection_id);
    }

    // First pre-trim roads if it's a consolidated intersection.
    for id in &intersection_roads {
        if let Some(endpt) = trim_roads_for_merging.get(&(id.osm_way_id, id.i1 == intersection_id))
        {
            let road = roads.get_mut(id).unwrap();
            if road.src_i == intersection_id {
                match road.trimmed_center_pts.safe_get_slice_starting_at(*endpt) {
                    Some(pl) => {
                        road.trimmed_center_pts = pl;
                    }
                    None => {
                        error!("{id}'s trimmed points start past the endpt {endpt}");
                        // Just skip. See https://github.com/a-b-street/abstreet/issues/654 for a
                        // start to diagnose. Repro at https://www.openstreetmap.org/node/53211693.
                    }
                }
            } else {
                assert_eq!(road.dst_i, intersection_id);
                match road.trimmed_center_pts.safe_get_slice_ending_at(*endpt) {
                    Some(pl) => {
                        road.trimmed_center_pts = pl;
                    }
                    None => {
                        error!("{id}'s trimmed points end before the endpt {endpt}");
                    }
                }
            }
        }
    }

    let mut road_lines = Vec::new();
    let mut endpoints_for_center = Vec::new();
    for id in &intersection_roads {
        let r = &roads[id];

        let center_pl = if r.src_i == intersection_id {
            r.trimmed_center_pts.reversed()
        } else if r.dst_i == intersection_id {
            r.trimmed_center_pts.clone()
        } else {
            panic!(
                "Incident road {} doesn't have an endpoint at {}",
                id, intersection_id
            );
        };
        endpoints_for_center.push(center_pl.last_pt());
        road_lines.push(RoadLine {
            id: *id,
            // Filled out momentarily
            sorting_pt: Pt2D::zero(),
            fwd_pl: center_pl.shift_right(r.half_width)?,
            back_pl: center_pl.shift_left(r.half_width)?,
            center_pl,
        });
    }
    // In most cases, this will just be the same point repeated a few times, so Pt2D::center is a
    // no-op. But when we have pretrimmed roads, this is much closer to the real "center" of the
    // polygon we're attempting to create.
    let intersection_center = Pt2D::center(&endpoints_for_center);

    // Sort the polylines in clockwise order around the center. This is subtle --
    // https://a-b-street.github.io/docs/tech/map/geometry/index.html#sorting-revisited. When we
    // get this wrong, the resulting polygon looks like a "bowtie," because the order of the
    // intersection polygon's points follows this clockwise ordering of roads.
    //
    // We could use the point on each road center line farthest from the intersection center. But
    // when some of the roads bend around, this produces incorrect ordering. Try walking along that
    // center line a distance equal to the _shortest_ road.
    let shortest_center = road_lines
        .iter()
        .map(|r| r.center_pl.length())
        .min()
        .unwrap();
    for r in &mut road_lines {
        r.sorting_pt = r
            .center_pl
            .must_dist_along(r.center_pl.length() - shortest_center)
            .0;
    }
    road_lines.sort_by_key(|r| {
        r.sorting_pt
            .angle_to(intersection_center)
            .normalized_degrees() as i64
    });

    let mut debug = Vec::new();
    // Debug the sorted order.
    if true {
        debug.push((
            "center".to_string(),
            Circle::new(intersection_center, Distance::meters(1.0)).to_polygon(),
        ));
        for (idx, r) in road_lines.iter().enumerate() {
            debug.push((
                idx.to_string(),
                Circle::new(r.sorting_pt, Distance::meters(1.0)).to_polygon(),
            ));
            if let Ok(l) = Line::new(intersection_center, r.sorting_pt) {
                debug.push((idx.to_string(), l.make_polygons(Distance::meters(0.5))));
            }
        }
    }

    if road_lines.len() == 1 {
        return deadend(roads, intersection_id, &road_lines, debug);
    }
    let rollback = road_lines
        .iter()
        .map(|r| (r.id, roads[&r.id].trimmed_center_pts.clone()))
        .collect::<Vec<_>>();

    if !trim_roads_for_merging.is_empty() {
        pretrimmed_geometry(roads, intersection_id, &road_lines, debug)
    } else if let Some(result) =
        on_off_ramp(roads, intersection_id, road_lines.clone(), debug.clone())
    {
        Ok(result)
    } else {
        // on_off_ramp failed, so first restore lines
        for (r, trimmed_center_pts) in rollback {
            roads.get_mut(&r).unwrap().trimmed_center_pts = trimmed_center_pts;
        }
        generalized_trim_back(roads, intersection_id, &road_lines, debug)
    }
}

fn generalized_trim_back(
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: osm::NodeID,
    input_road_lines: &[RoadLine],
    mut debug: Vec<(String, Polygon)>,
) -> Result<(Polygon, Vec<(String, Polygon)>)> {
    let mut road_lines: Vec<(OriginalRoad, PolyLine)> = Vec::new();
    for r in input_road_lines {
        road_lines.push((r.id, r.fwd_pl.clone()));
        road_lines.push((r.id, r.back_pl.clone()));

        if false {
            debug.push((
                format!("{} fwd", r.id.osm_way_id),
                r.fwd_pl.make_polygons(Distance::meters(1.0)),
            ));
            debug.push((
                format!("{} back", r.id.osm_way_id),
                r.back_pl.make_polygons(Distance::meters(1.0)),
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
                bail!(
                    "{} and {} have overlapping segments. You likely need to fix OSM and make the \
                     two ways meet at exactly one node.",
                    r1,
                    r2
                );
            }

            // Sometimes two road PLs may hit at multiple points because they're thick and close
            // together. pl1.intersection(pl2) returns the "first" hit from pl1's
            // perspective, so reverse it, ensuring we find the hit closest to the
            // intersection we're working on.
            // TODO I hoped this would subsume the second_half() hack above, but it sadly doesn't.
            if let Some((hit, angle)) = use_pl1.reversed().intersection(&use_pl2) {
                // Find where the perpendicular hits the original road line
                let perp = InfiniteLine::from_pt_angle(hit, angle.rotate_degs(90.0));
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
                    warn!(
                        "{} and {} hit, but the perpendicular never hit the original center line, \
                         or the trimmed thing is empty",
                        r1, r2
                    );
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
    for idx in 0..input_road_lines.len() as isize {
        let (id, fwd_pl, back_pl) = {
            let r = wraparound_get(input_road_lines, idx);
            (r.id, &r.fwd_pl, &r.back_pl)
        };
        // TODO Ahhh these names are confusing. Adjacent to the fwd_pl, but it's a back pl.
        let adj_back_pl = &wraparound_get(input_road_lines, idx + 1).fwd_pl;
        let adj_fwd_pl = &wraparound_get(input_road_lines, idx - 1).back_pl;

        roads.get_mut(&id).unwrap().trimmed_center_pts = new_road_centers[&id].clone();
        let r = &roads[&id];

        // Include collisions between polylines of adjacent roads, so the polygon doesn't cover area
        // not originally covered by the thick road bands.
        // Always take the second_half here to handle roads that intersect at multiple points.
        // TODO Should maybe do reversed() to fwd_pl here too. And why not make all the lines
        // passed in point AWAY from the intersection instead?
        if fwd_pl.length() >= EPSILON_DIST * 3.0 && adj_fwd_pl.length() >= EPSILON_DIST * 3.0 {
            if let Some((hit, _)) = fwd_pl.second_half().intersection(&adj_fwd_pl.second_half()) {
                endpoints.push(hit);
            }
        } else {
            warn!(
                "Excluding collision between original polylines of {} and something, because \
                 stuff's too short",
                id
            );
        }

        // Shift those final centers out again to find the main endpoints for the polygon.
        if r.dst_i == i {
            endpoints.push(r.trimmed_center_pts.shift_right(r.half_width)?.last_pt());
            endpoints.push(r.trimmed_center_pts.shift_left(r.half_width)?.last_pt());
        } else {
            endpoints.push(r.trimmed_center_pts.shift_left(r.half_width)?.first_pt());
            endpoints.push(r.trimmed_center_pts.shift_right(r.half_width)?.first_pt());
        }

        if back_pl.length() >= EPSILON_DIST * 3.0 && adj_back_pl.length() >= EPSILON_DIST * 3.0 {
            if let Some((hit, _)) = back_pl
                .second_half()
                .intersection(&adj_back_pl.second_half())
            {
                endpoints.push(hit);
            }
        } else {
            warn!(
                "Excluding collision between original polylines of {} and something, because \
                 stuff's too short",
                id
            );
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
        Ok((Ring::must_new(main_result).into_polygon(), debug))
    } else {
        warn!(
            "{}'s polygon has weird repeats, forcibly removing points",
            i
        );
        Ok((Ring::must_new(deduped).into_polygon(), debug))
    }

    // TODO Or always sort points? Helps some cases, hurts other for downtown Seattle.
    /*endpoints.sort_by_key(|pt| pt.to_hashable());
    endpoints = Pt2D::approx_dedupe(endpoints, Distance::meters(0.1));
    let center = Pt2D::center(&endpoints);
    endpoints.sort_by_key(|pt| pt.angle_to(center).normalized_degrees() as i64);
    (close_off_polygon(endpoints), debug)*/
}

fn pretrimmed_geometry(
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: osm::NodeID,
    road_lines: &[RoadLine],
    debug: Vec<(String, Polygon)>,
) -> Result<(Polygon, Vec<(String, Polygon)>)> {
    let mut endpoints: Vec<Pt2D> = Vec::new();
    for r in road_lines {
        let r = &roads[&r.id];
        // Shift those final centers out again to find the main endpoints for the polygon.
        if r.dst_i == i {
            endpoints.push(r.trimmed_center_pts.shift_right(r.half_width)?.last_pt());
            endpoints.push(r.trimmed_center_pts.shift_left(r.half_width)?.last_pt());
        } else {
            endpoints.push(r.trimmed_center_pts.shift_left(r.half_width)?.first_pt());
            endpoints.push(r.trimmed_center_pts.shift_right(r.half_width)?.first_pt());
        }
    }

    // TODO Do all of the crazy deduping that generalized_trim_back does?
    let result = Ring::new(close_off_polygon(Pt2D::approx_dedupe(
        endpoints,
        Distance::meters(0.1),
    )))?;
    Ok((result.into_polygon(), debug))
}

fn deadend(
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: osm::NodeID,
    road_lines: &[RoadLine],
    debug: Vec<(String, Polygon)>,
) -> Result<(Polygon, Vec<(String, Polygon)>)> {
    let len = DEGENERATE_INTERSECTION_HALF_LENGTH * 4.0;

    let id = road_lines[0].id;
    let mut pl_a = road_lines[0].fwd_pl.clone();
    let mut pl_b = road_lines[0].back_pl.clone();
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
    } else if r.src_i == i {
        r.trimmed_center_pts.extend_to_length(len_with_buffer)
    } else {
        r.trimmed_center_pts
            .reversed()
            .extend_to_length(len_with_buffer)
            .reversed()
    };

    // After trimming the center points, the two sides of the road may be at different
    // points, so shift the center out again to find the endpoints.
    // TODO Refactor with generalized_trim_back.
    let mut endpts = vec![pl_b.last_pt(), pl_a.last_pt()];
    if r.dst_i == i {
        endpts.push(trimmed.shift_right(r.half_width)?.last_pt());
        endpts.push(trimmed.shift_left(r.half_width)?.last_pt());
    } else {
        endpts.push(trimmed.shift_left(r.half_width)?.first_pt());
        endpts.push(trimmed.shift_right(r.half_width)?.first_pt());
    }

    endpts.dedup();
    Ok((
        Ring::must_new(close_off_polygon(endpts)).into_polygon(),
        debug,
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

// The normal generalized_trim_back approach produces huge intersections when 3 roads meet at
// certain angles. It usually happens for highway on/off ramps. Try something different here. In
// lieu of proper docs, see https://twitter.com/CarlinoDustin/status/1290799086036111360.
fn on_off_ramp(
    roads: &mut BTreeMap<OriginalRoad, Road>,
    i: osm::NodeID,
    road_lines: Vec<RoadLine>,
    mut debug: Vec<(String, Polygon)>,
) -> Option<(Polygon, Vec<(String, Polygon)>)> {
    if road_lines.len() != 3 {
        return None;
    }
    // TODO Really this should apply based on some geometric consideration (one of the endpoints
    // totally inside the other thick road's polygon), but for the moment, this is an OK filter.
    //
    // Example candidate: https://www.openstreetmap.org/node/32177767
    let mut ok = false;
    for r in &road_lines {
        if roads[&r.id].osm_tags.is_any(
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

    let mut pieces = Vec::new();
    // TODO Use this abstraction for all the code here?
    for r in road_lines {
        let id = r.id;
        let right = r.fwd_pl;
        let left = r.back_pl;
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

    // Break ties by preferring the outbound roads for thin
    pieces.sort_by_key(|r| (roads[&r.id].half_width, r.id.i2 == i));
    let thick1 = pieces.pop().unwrap();
    let thick2 = pieces.pop().unwrap();
    let thin = pieces.pop().unwrap();

    // Find where the thin hits the thick farthest along.
    // (trimmed thin center, trimmed thick center, the thick road we hit)
    let mut best_hit: Option<(PolyLine, PolyLine, OriginalRoad)> = None;
    for thin_pl in [&thin.left, &thin.right] {
        for thick in [&thick1, &thick2] {
            for thick_pl in [&thick.left, &thick.right] {
                if thin_pl == thick_pl {
                    // How? Just bail.
                    return None;
                }
                if let Some((hit, angle)) = thin_pl.intersection(thick_pl) {
                    // Find where the perpendicular hits the original road line
                    // TODO Refactor something to go from a hit+angle on a left/right to a trimmed
                    // center.
                    let perp = InfiniteLine::from_pt_angle(hit, angle.rotate_degs(90.0));
                    let trimmed_thin = thin
                        .center
                        .reversed()
                        .intersection_infinite(&perp)
                        .and_then(|trim_to| thin.center.get_slice_ending_at(trim_to))?;

                    // Do the same for the thick road
                    let (_, angle) = thick_pl.dist_along_of_point(hit)?;
                    let perp = InfiniteLine::from_pt_angle(hit, angle.rotate_degs(90.0));
                    let trimmed_thick = thick
                        .center
                        .reversed()
                        .intersection_infinite(&perp)
                        .and_then(|trim_to| thick.center.get_slice_ending_at(trim_to))?;

                    if false {
                        debug.push((
                            "1".to_string(),
                            Circle::new(hit, Distance::meters(3.0)).to_polygon(),
                        ));
                        debug.push((
                            "2".to_string(),
                            Circle::new(trimmed_thin.last_pt(), Distance::meters(3.0)).to_polygon(),
                        ));
                        debug.push((
                            "3".to_string(),
                            Circle::new(trimmed_thick.last_pt(), Distance::meters(3.0))
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

        // Trim the thick extra ends at the intersection
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
    for id in [thin.id, thick1.id, thick2.id] {
        let r = &roads[&id];
        // Shift those final centers out again to find the main endpoints for the polygon.
        if r.dst_i == i {
            endpoints.push(
                r.trimmed_center_pts
                    .shift_right(r.half_width)
                    .ok()?
                    .last_pt(),
            );
            endpoints.push(
                r.trimmed_center_pts
                    .shift_left(r.half_width)
                    .ok()?
                    .last_pt(),
            );
        } else {
            endpoints.push(
                r.trimmed_center_pts
                    .shift_left(r.half_width)
                    .ok()?
                    .first_pt(),
            );
            endpoints.push(
                r.trimmed_center_pts
                    .shift_right(r.half_width)
                    .ok()?
                    .first_pt(),
            );
        }
    }
    /*for (idx, pt) in endpoints.iter().enumerate() {
        debug.push((format!("{}", idx), Circle::new(*pt, Distance::meters(2.0)).to_polygon()));
    }*/

    endpoints.sort_by_key(|pt| pt.to_hashable());
    endpoints.dedup();
    let center = Pt2D::center(&endpoints);
    endpoints.sort_by_key(|pt| pt.angle_to(center).normalized_degrees() as i64);
    endpoints.dedup();
    Some((
        Ring::must_new(close_off_polygon(endpoints)).into_polygon(),
        debug,
    ))

    //let dummy = Circle::new(orig_lines[0].3.last_pt(), Distance::meters(3.0)).to_polygon();
    //Some((close_off_polygon(dummy.into_points()), debug))
}

use crate::make::match_points_to_lanes;
use crate::raw::RawParkingLot;
use crate::{
    osm, Map, ParkingLot, ParkingLotID, PathConstraints, Position, NORMAL_LANE_THICKNESS,
    PARKING_LOT_SPOT_LENGTH,
};
use abstutil::Timer;
use geom::{Angle, Distance, FindClosest, HashablePt2D, Line, PolyLine, Polygon, Pt2D, Ring};
use std::collections::HashSet;

pub fn make_all_parking_lots(
    input: &Vec<RawParkingLot>,
    aisles: &Vec<(osm::WayID, Vec<Pt2D>)>,
    map: &Map,
    timer: &mut Timer,
) -> Vec<ParkingLot> {
    timer.start("convert parking lots");
    let mut center_per_lot: Vec<HashablePt2D> = Vec::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    for lot in input {
        let center = lot.polygon.center().to_hashable();
        center_per_lot.push(center);
        query.insert(center);
    }

    let sidewalk_buffer = Distance::meters(7.5);
    let driveway_buffer = Distance::meters(7.0);
    let sidewalk_pts = match_points_to_lanes(
        map.get_bounds(),
        query,
        map.all_lanes(),
        |l| l.is_walkable(),
        sidewalk_buffer,
        Distance::meters(1000.0),
        timer,
    );

    let mut results = Vec::new();
    let mut osm_tags = Vec::new();
    timer.start_iter("create parking lot driveways", center_per_lot.len());
    for (lot_center, orig) in center_per_lot.into_iter().zip(input.iter()) {
        timer.next();
        // TODO Refactor this
        if let Some(sidewalk_pos) = sidewalk_pts.get(&lot_center) {
            let sidewalk_line = match Line::new(lot_center.to_pt2d(), sidewalk_pos.pt(map)) {
                Some(l) => trim_path(&orig.polygon, l),
                None => {
                    timer.warn(format!(
                        "Skipping parking lot {} because front path has 0 length",
                        orig.osm_id
                    ));
                    continue;
                }
            };

            // Can this lot have a driveway? If it's not next to a driving lane, then no.
            let mut driveway: Option<(PolyLine, Position)> = None;
            let sidewalk_lane = sidewalk_pos.lane();
            if let Some(driving_pos) = map
                .get_parent(sidewalk_lane)
                .find_closest_lane(sidewalk_lane, |l| PathConstraints::Car.can_use(l, map), map)
                .and_then(|l| {
                    sidewalk_pos
                        .equiv_pos(l, map)
                        .buffer_dist(driveway_buffer, map)
                })
            {
                driveway = Some((
                    PolyLine::must_new(vec![
                        sidewalk_line.pt1(),
                        sidewalk_line.pt2(),
                        driving_pos.pt(map),
                    ]),
                    driving_pos,
                ));
            }
            if let Some((driveway_line, driving_pos)) = driveway {
                let id = ParkingLotID(results.len());
                results.push(ParkingLot {
                    id,
                    polygon: orig.polygon.clone(),
                    aisles: Vec::new(),
                    osm_id: orig.osm_id,
                    spots: Vec::new(),
                    extra_spots: 0,

                    driveway_line,
                    driving_pos,
                    sidewalk_line,
                    sidewalk_pos: *sidewalk_pos,
                });
                osm_tags.push(&orig.osm_tags);
            } else {
                timer.warn(format!(
                    "Parking lot from {}, near sidewalk {}, can't have a driveway.",
                    orig.osm_id,
                    sidewalk_pos.lane()
                ));
            }
        }
    }
    timer.note(format!(
        "Discarded {} parking lots that weren't close enough to a sidewalk",
        input.len() - results.len()
    ));

    let mut closest: FindClosest<ParkingLotID> = FindClosest::new(map.get_bounds());
    for lot in &results {
        closest.add(lot.id, lot.polygon.points());
    }
    timer.start_iter("match parking aisles", aisles.len());
    for (aisle_id, pts) in aisles {
        timer.next();
        // Use the center of all the aisle points to match it to a lot
        let candidates: Vec<ParkingLotID> = closest
            .all_close_pts(Pt2D::center(&pts), Distance::meters(500.0))
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();

        match Ring::split_points(pts) {
            Ok((polylines, rings)) => {
                'PL: for pl in polylines {
                    for id in &candidates {
                        let lot = &mut results[id.0];
                        if let Some(segment) = lot.polygon.clip_polyline(&pl) {
                            lot.aisles.push(segment);
                            continue 'PL;
                        }
                    }
                }
                'RING: for ring in rings {
                    for id in &candidates {
                        let lot = &mut results[id.0];
                        if let Some(segment) = lot.polygon.clip_ring(&ring) {
                            lot.aisles.push(segment);
                            continue 'RING;
                        }
                    }
                }
            }
            Err(err) => {
                timer.warn(format!(
                    "Parking aisle {} has weird geometry: {}",
                    aisle_id, err
                ));
            }
        }
    }

    timer.start_iter("generate parking lot spots", results.len());
    for (lot, osm_tags) in results.iter_mut().zip(osm_tags.into_iter()) {
        timer.next();
        lot.spots = infer_spots(&lot.polygon, &lot.aisles);

        // Guess how many extra spots are available, that maybe aren't renderable.
        if lot.spots.is_empty() {
            // No parking aisles. Just guess based on the area. One spot per 30m^2 is a quick guess
            // from looking at examples with aisles.
            lot.extra_spots = (lot.polygon.area() / 30.0) as usize;
        }
        // For multi-story garages, assume every floor has the same capacity.
        if let Some(levels) = osm_tags
            .get("building:levels")
            .and_then(|x| x.parse::<usize>().ok())
        {
            let base = lot.spots.len().max(lot.extra_spots);
            lot.extra_spots = base * levels;
        }
    }

    timer.stop("convert parking lots");

    results
}

// Adjust the path to start on the building's border, not center
fn trim_path(poly: &Polygon, path: Line) -> Line {
    for bldg_line in poly.points().windows(2) {
        if let Some(l1) = Line::new(bldg_line[0], bldg_line[1]) {
            if let Some(hit) = l1.intersection(&path) {
                if let Some(l2) = Line::new(hit, path.pt2()) {
                    return l2;
                }
            }
        }
    }
    // Just give up
    path
}

fn infer_spots(lot_polygon: &Polygon, aisles: &Vec<Vec<Pt2D>>) -> Vec<(Pt2D, Angle)> {
    let mut spots = Vec::new();
    let mut finalized_lines = Vec::new();

    for aisle in aisles {
        let aisle_thickness = NORMAL_LANE_THICKNESS / 2.0;
        let pl = PolyLine::unchecked_new(aisle.clone());

        for rotate in vec![90.0, -90.0] {
            // Blindly generate all of the lines
            let lines = {
                let mut lines = Vec::new();
                let mut start = Distance::ZERO;
                while start + NORMAL_LANE_THICKNESS < pl.length() {
                    let (pt, angle) = pl.must_dist_along(start);
                    start += NORMAL_LANE_THICKNESS;
                    let theta = angle.rotate_degs(rotate);
                    lines.push(Line::must_new(
                        pt.project_away(aisle_thickness / 2.0, theta),
                        pt.project_away(aisle_thickness / 2.0 + PARKING_LOT_SPOT_LENGTH, theta),
                    ));
                }
                lines
            };

            for pair in lines.windows(2) {
                let l1 = &pair[0];
                let l2 = &pair[1];
                if let Some(back) = Line::new(l1.pt2(), l2.pt2()) {
                    if l1.intersection(&l2).is_none()
                        && l1.angle().approx_eq(l2.angle(), 5.0)
                        && line_valid(lot_polygon, aisles, l1, &finalized_lines)
                        && line_valid(lot_polygon, aisles, l2, &finalized_lines)
                        && line_valid(lot_polygon, aisles, &back, &finalized_lines)
                    {
                        let avg_angle = (l1.angle() + l2.angle()) / 2.0;
                        spots.push((back.middle().unwrap(), avg_angle.opposite()));
                        finalized_lines.push(l1.clone());
                        finalized_lines.push(l2.clone());
                        finalized_lines.push(back);
                    }
                }
            }
        }
    }
    spots
}

fn line_valid(
    lot_polygon: &Polygon,
    aisles: &Vec<Vec<Pt2D>>,
    line: &Line,
    finalized_lines: &Vec<Line>,
) -> bool {
    // Don't leak out of the parking lot
    // TODO Entire line
    if !lot_polygon.contains_pt(line.pt1()) || !lot_polygon.contains_pt(line.pt2()) {
        return false;
    }

    // Don't let this line hit another line
    if finalized_lines.iter().any(|other| line.crosses(other)) {
        return false;
    }

    // Don't hit an aisle
    if aisles.iter().any(|pts| {
        PolyLine::unchecked_new(pts.clone())
            .intersection(&line.to_polyline())
            .is_some()
    }) {
        return false;
    }

    true
}

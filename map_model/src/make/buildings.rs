use crate::make::match_points_to_lanes;
use crate::raw::{OriginalBuilding, RawBuilding, RawParkingLot};
use crate::{
    osm, Building, BuildingID, FrontPath, LaneID, LaneType, Map, OffstreetParking, ParkingLot,
    ParkingLotID, Position, NORMAL_LANE_THICKNESS, PARKING_LOT_SPOT_LENGTH,
};
use abstutil::Timer;
use geom::{Angle, Distance, FindClosest, HashablePt2D, Line, PolyLine, Polygon, Pt2D, Ring};
use std::collections::{BTreeMap, HashSet};

pub fn make_all_buildings(
    input: &BTreeMap<OriginalBuilding, RawBuilding>,
    map: &Map,
    timer: &mut Timer,
) -> Vec<Building> {
    timer.start("convert buildings");
    let mut center_per_bldg: BTreeMap<OriginalBuilding, HashablePt2D> = BTreeMap::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    timer.start_iter("get building center points", input.len());
    for (id, b) in input {
        timer.next();
        let center = b.polygon.center().to_hashable();
        center_per_bldg.insert(*id, center);
        query.insert(center);
    }

    // equiv_pos could be a little closer, so use two buffers
    let sidewalk_buffer = Distance::meters(7.5);
    let driveway_buffer = Distance::meters(7.0);
    let sidewalk_pts = match_points_to_lanes(
        map.get_bounds(),
        query,
        map.all_lanes(),
        |l| l.is_sidewalk(),
        // Don't put connections too close to intersections
        sidewalk_buffer,
        // Try not to skip any buildings, but more than 1km from a sidewalk is a little much
        Distance::meters(1000.0),
        timer,
    );

    let mut results = Vec::new();
    timer.start_iter("create building front paths", center_per_bldg.len());
    for (orig_id, bldg_center) in center_per_bldg {
        timer.next();
        if let Some(sidewalk_pos) = sidewalk_pts.get(&bldg_center) {
            let b = &input[&orig_id];
            let sidewalk_line = match Line::new(bldg_center.to_pt2d(), sidewalk_pos.pt(map)) {
                Some(l) => trim_path(&b.polygon, l),
                None => {
                    timer.warn(format!(
                        "Skipping building {} because front path has 0 length",
                        orig_id
                    ));
                    continue;
                }
            };

            let id = BuildingID(results.len());
            let mut bldg = Building {
                id,
                polygon: b.polygon.clone(),
                address: get_address(&b.osm_tags, sidewalk_pos.lane(), map),
                name: b.osm_tags.get(osm::NAME).cloned(),
                osm_way_id: orig_id.osm_way_id,
                front_path: FrontPath {
                    sidewalk: *sidewalk_pos,
                    line: sidewalk_line.clone(),
                },
                amenities: b.amenities.clone(),
                parking: None,
                label_center: b.polygon.polylabel(),
            };

            // Can this building have a driveway? If it's not next to a driving lane, then no.
            let sidewalk_lane = sidewalk_pos.lane();
            if let Ok(driving_lane) = map
                .get_parent(sidewalk_lane)
                .find_closest_lane(sidewalk_lane, vec![LaneType::Driving])
            {
                let driving_pos = sidewalk_pos.equiv_pos(driving_lane, Distance::ZERO, map);

                // This shouldn't fail much anymore, unless equiv_pos winds up being pretty
                // different
                if driving_pos.dist_along() > driveway_buffer
                    && map.get_l(driving_lane).length() - driving_pos.dist_along() > driveway_buffer
                {
                    let driveway_line = PolyLine::must_new(vec![
                        sidewalk_line.pt1(),
                        sidewalk_line.pt2(),
                        driving_pos.pt(map),
                    ]);
                    bldg.parking = Some(OffstreetParking {
                        public_garage_name: b.public_garage_name.clone(),
                        num_spots: b.num_parking_spots,
                        driveway_line,
                        driving_pos,
                    });
                }
            }
            if bldg.parking.is_none() {
                timer.warn(format!(
                    "{} can't have a driveway. Forfeiting {} parking spots",
                    bldg.id, b.num_parking_spots
                ));
            }

            results.push(bldg);
        }
    }

    timer.note(format!(
        "Discarded {} buildings that weren't close enough to a sidewalk",
        input.len() - results.len()
    ));
    timer.stop("convert buildings");

    results
}

pub fn make_all_parking_lots(
    input: &Vec<RawParkingLot>,
    aisles: &Vec<Vec<Pt2D>>,
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
        |l| l.is_sidewalk(),
        sidewalk_buffer,
        Distance::meters(1000.0),
        timer,
    );

    let mut results = Vec::new();
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
            if let Ok(driving_lane) = map
                .get_parent(sidewalk_lane)
                .find_closest_lane(sidewalk_lane, vec![LaneType::Driving])
            {
                let driving_pos = sidewalk_pos.equiv_pos(driving_lane, Distance::ZERO, map);

                if driving_pos.dist_along() > driveway_buffer
                    && map.get_l(driving_lane).length() - driving_pos.dist_along() > driveway_buffer
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
            }
            if let Some((driveway_line, driving_pos)) = driveway {
                let id = ParkingLotID(results.len());
                results.push(ParkingLot {
                    id,
                    polygon: orig.polygon.clone(),
                    aisles: Vec::new(),
                    osm_id: orig.osm_id,
                    spots: Vec::new(),

                    driveway_line,
                    driving_pos,
                    sidewalk_line,
                    sidewalk_pos: *sidewalk_pos,
                });
            } else {
                timer.warn(format!(
                    "Parking lot from OSM way {} can't have a driveway.",
                    orig.osm_id
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
    for pts in aisles {
        timer.next();
        // Use the center of all the aisle points to match it to a lot
        let candidates: Vec<ParkingLotID> = closest
            .all_close_pts(Pt2D::center(&pts), Distance::meters(500.0))
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();

        let (polylines, rings) = Ring::split_points(pts).unwrap();
        'PL: for pl in polylines {
            for id in &candidates {
                let lot = &mut results[id.0];
                for segment in lot.polygon.clip_polyline(&pl) {
                    lot.aisles.push(segment);
                    continue 'PL;
                }
            }
        }
        'RING: for ring in rings {
            for id in &candidates {
                let lot = &mut results[id.0];
                for segment in lot.polygon.clip_ring(&ring) {
                    lot.aisles.push(segment);
                    continue 'RING;
                }
            }
        }
    }

    timer.start_iter("generate parking lot spots", results.len());
    for lot in results.iter_mut() {
        timer.next();
        lot.spots = infer_spots(&lot.polygon, &lot.aisles);
    }

    timer.stop("convert parking lots");

    results
}

// Adjust the path to start on the building's border, not center
fn trim_path(poly: &Polygon, path: Line) -> Line {
    for bldg_line in poly.points().windows(2) {
        let l = Line::must_new(bldg_line[0], bldg_line[1]);
        if let Some(hit) = l.intersection(&path) {
            if let Some(l) = Line::new(hit, path.pt2()) {
                return l;
            }
        }
    }
    // Just give up
    path
}

fn get_address(tags: &BTreeMap<String, String>, sidewalk: LaneID, map: &Map) -> String {
    match (tags.get("addr:housenumber"), tags.get("addr:street")) {
        (Some(num), Some(st)) => format!("{} {}", num, st),
        (None, Some(st)) => format!("??? {}", st),
        _ => format!("??? {}", map.get_parent(sidewalk).get_name()),
    }
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
                    let (pt, angle) = pl.dist_along(start);
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
                let back = Line::must_new(l1.pt2(), l2.pt2());
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

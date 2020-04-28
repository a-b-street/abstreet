use crate::make::sidewalk_finder::find_sidewalk_points;
use crate::raw::{OriginalBuilding, RawBuilding};
use crate::{Building, BuildingID, FrontPath, LaneType, Map, OffstreetParking};
use abstutil::Timer;
use geom::{Distance, HashablePt2D, Line, PolyLine, Polygon};
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

    // Skip buildings that're too far away from their sidewalk
    let sidewalk_pts = find_sidewalk_points(
        map.get_bounds(),
        query,
        map.all_lanes(),
        Distance::meters(100.0),
        timer,
    );

    let mut results = Vec::new();
    timer.start_iter("create building front paths", center_per_bldg.len());
    for (orig_id, bldg_center) in center_per_bldg {
        timer.next();
        if let Some(sidewalk_pos) = sidewalk_pts.get(&bldg_center) {
            let sidewalk_pt = sidewalk_pos.pt(map);
            if sidewalk_pt == bldg_center.to_pt2d() {
                timer.warn(format!(
                    "Skipping building {} because front path has 0 length",
                    orig_id
                ));
                continue;
            }
            let b = &input[&orig_id];
            let sidewalk_line =
                trim_path(&b.polygon, Line::new(bldg_center.to_pt2d(), sidewalk_pt));

            let id = BuildingID(results.len());
            let mut bldg = Building {
                id,
                polygon: b.polygon.clone(),
                osm_tags: b.osm_tags.clone(),
                osm_way_id: orig_id.osm_way_id,
                front_path: FrontPath {
                    sidewalk: *sidewalk_pos,
                    line: sidewalk_line.clone(),
                },
                amenities: b.amenities.clone(),
                parking: None,
                label_center: b.polygon.polylabel(),
            };

            // Can this building have a driveway? If it's not next to a driving lane part of the
            // main connectivity graph, then no.
            let sidewalk_lane = sidewalk_pos.lane();
            if let Ok(driving_lane) = map
                .get_parent(sidewalk_lane)
                .find_closest_lane(sidewalk_lane, vec![LaneType::Driving])
            {
                if map.get_l(driving_lane).parking_blackhole.is_none() {
                    let driving_pos = sidewalk_pos.equiv_pos(driving_lane, Distance::ZERO, map);

                    let buffer = Distance::meters(7.0);
                    if driving_pos.dist_along() > buffer
                        && map.get_l(driving_lane).length() - driving_pos.dist_along() > buffer
                    {
                        let driveway_line = PolyLine::new(vec![
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

    let discarded = input.len() - results.len();
    if discarded > 0 {
        timer.note(format!(
            "Discarded {} buildings that weren't close enough to a sidewalk",
            discarded
        ));
    }
    timer.stop("convert buildings");

    results
}

// Adjust the path to start on the building's border, not center
fn trim_path(poly: &Polygon, path: Line) -> Line {
    for bldg_line in poly.points().windows(2) {
        let l = Line::new(bldg_line[0], bldg_line[1]);
        if let Some(hit) = l.intersection(&path) {
            if let Some(l) = Line::maybe_new(hit, path.pt2()) {
                return l;
            }
        }
    }
    // Just give up
    path
}

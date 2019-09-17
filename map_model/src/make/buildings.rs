use crate::make::sidewalk_finder::find_sidewalk_points;
use crate::{raw_data, Building, BuildingID, FrontPath, Lane, LaneID, Position, Road};
use abstutil::Timer;
use geom::{Bounds, Distance, FindClosest, HashablePt2D, Line, Polygon};
use std::collections::{BTreeMap, HashSet};

pub fn make_all_buildings(
    results: &mut Vec<Building>,
    input: &BTreeMap<raw_data::StableBuildingID, raw_data::Building>,
    bounds: &Bounds,
    lanes: &Vec<Lane>,
    roads: &Vec<Road>,
    timer: &mut Timer,
) {
    timer.start("convert buildings");
    let mut center_per_bldg: BTreeMap<raw_data::StableBuildingID, HashablePt2D> = BTreeMap::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    timer.start_iter("get building center points", input.len());
    for (id, b) in input {
        timer.next();
        // TODO Use the polylabel? Want to have visually distinct lines for front path and
        // driveway; using two different "centers" is a lazy way for now.
        let center = b.polygon.center().to_hashable();
        center_per_bldg.insert(*id, center);
        query.insert(center);
    }

    let mut closest_driving: FindClosest<LaneID> = FindClosest::new(bounds);
    for l in lanes {
        // TODO And is the rightmost driving lane...
        if !l.is_driving() {
            continue;
        }
        let tags = &roads[l.parent.0].osm_tags;
        if tags.get("highway") == Some(&"motorway".to_string())
            || tags.get("tunnel") == Some(&"yes".to_string())
        {
            continue;
        }

        closest_driving.add(l.id, l.lane_center_pts.points());
    }

    // Skip buildings that're too far away from their sidewalk
    let sidewalk_pts = find_sidewalk_points(bounds, query, lanes, Distance::meters(100.0), timer);

    timer.start_iter("create building front paths", center_per_bldg.len());
    for (stable_id, bldg_center) in center_per_bldg {
        timer.next();
        if let Some(sidewalk_pos) = sidewalk_pts.get(&bldg_center) {
            let sidewalk_pt = lanes[sidewalk_pos.lane().0]
                .dist_along(sidewalk_pos.dist_along())
                .0;
            if sidewalk_pt.epsilon_eq(bldg_center.to_pt2d()) {
                timer.warn("Skipping a building because front path has 0 length".to_string());
                continue;
            }
            let b = &input[&stable_id];
            let line = trim_path(&b.polygon, Line::new(bldg_center.to_pt2d(), sidewalk_pt));

            let id = BuildingID(results.len());
            let mut bldg = Building {
                id,
                polygon: b.polygon.clone(),
                osm_tags: b.osm_tags.clone(),
                osm_way_id: b.osm_way_id,
                front_path: FrontPath {
                    sidewalk: *sidewalk_pos,
                    line,
                },
                parking: b.parking.clone(),
                label_center: b.polygon.polylabel(),
            };

            // Make a driveway from the parking icon to the nearest road.
            if let Some(ref mut p) = bldg.parking {
                // TODO Is it a problem if the driveway is too close to the start/end of a lane?
                let (driving_lane, driving_pt) = closest_driving
                    .closest_pt(bldg.label_center, Distance::meters(100.0))
                    .expect("Can't find driveway!");
                let dist_along = lanes[driving_lane.0]
                    .dist_along_of_point(driving_pt)
                    .expect("Can't find dist_along_of_point for driveway");
                p.driveway_line =
                    trim_path(&bldg.polygon, Line::new(bldg.label_center, driving_pt));
                if p.driveway_line.length() < Distance::meters(1.0) {
                    timer.warn(format!(
                        "Driveway of {} is very short: {}",
                        bldg.id,
                        p.driveway_line.length()
                    ));
                }
                p.driving_pos = Position::new(driving_lane, dist_along);
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

use crate::make::sidewalk_finder::find_sidewalk_points;
use crate::{raw_data, Building, BuildingID, BuildingType, FrontPath, Lane};
use abstutil::Timer;
use geom::{Bounds, Distance, GPSBounds, HashablePt2D, Line, Pt2D};
use std::collections::{BTreeMap, HashSet};

pub fn make_all_buildings(
    results: &mut Vec<Building>,
    input: &Vec<raw_data::Building>,
    gps_bounds: &GPSBounds,
    bounds: &Bounds,
    lanes: &Vec<Lane>,
    timer: &mut Timer,
) {
    timer.start("convert buildings");
    let mut pts_per_bldg: Vec<Vec<Pt2D>> = Vec::new();
    let mut center_per_bldg: Vec<HashablePt2D> = Vec::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    timer.start_iter("get building center points", input.len());
    for b in input {
        timer.next();
        let pts = Pt2D::approx_dedupe(
            b.points
                .iter()
                .map(|coord| Pt2D::from_gps(*coord, gps_bounds).unwrap())
                .collect(),
            geom::EPSILON_DIST,
        );
        let center: HashablePt2D = Pt2D::center(&pts).into();
        pts_per_bldg.push(pts);
        center_per_bldg.push(center);
        query.insert(center);
    }

    // Skip buildings that're too far away from their sidewalk
    let sidewalk_pts = find_sidewalk_points(bounds, query, lanes, Distance::meters(100.0), timer);

    timer.start_iter("create building front paths", pts_per_bldg.len());
    for (idx, points) in pts_per_bldg.into_iter().enumerate() {
        timer.next();
        let bldg_center = center_per_bldg[idx];
        if let Some(sidewalk_pos) = sidewalk_pts.get(&bldg_center) {
            let sidewalk_pt = lanes[sidewalk_pos.lane().0]
                .dist_along(sidewalk_pos.dist_along())
                .0;
            if sidewalk_pt.epsilon_eq(bldg_center.into()) {
                warn!("Skipping a building because front path has 0 length");
                continue;
            }
            let line = trim_front_path(&points, Line::new(bldg_center.into(), sidewalk_pt));

            let id = BuildingID(results.len());
            results.push(Building {
                id,
                building_type: classify(input[idx].num_residential_units, &input[idx].osm_tags),
                points,
                osm_tags: input[idx].osm_tags.clone(),
                osm_way_id: input[idx].osm_way_id,
                front_path: FrontPath {
                    bldg: id,
                    sidewalk: *sidewalk_pos,
                    line,
                },
                num_residential_units: input[idx].num_residential_units,
            });
        }
    }

    let discarded = input.len() - results.len();
    if discarded > 0 {
        info!(
            "Discarded {} buildings that weren't close enough to a sidewalk",
            discarded
        );
    }
    timer.stop("convert buildings");
}

// Adjust the path to start on the building's border, not center
fn trim_front_path(bldg_points: &Vec<Pt2D>, path: Line) -> Line {
    for bldg_line in bldg_points.windows(2) {
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

fn classify(num_residential_units: Option<usize>, tags: &BTreeMap<String, String>) -> BuildingType {
    if num_residential_units.is_some() {
        return BuildingType::Residence;
    }
    if tags.get("building") == Some(&"apartments".to_string()) {
        return BuildingType::Residence;
    }
    if tags.get("building") == Some(&"residential".to_string()) {
        return BuildingType::Residence;
    }
    if tags.get("building") == Some(&"house".to_string()) {
        return BuildingType::Residence;
    }

    if tags.contains_key(&"shop".to_string()) || tags.contains_key(&"amenity".to_string()) {
        return BuildingType::Business;
    }
    if tags.get("building") == Some(&"commercial".to_string()) {
        return BuildingType::Business;
    }
    if tags.get("building") == Some(&"retail".to_string()) {
        return BuildingType::Business;
    }

    BuildingType::Unknown
}

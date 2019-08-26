use crate::make::sidewalk_finder::find_sidewalk_points;
use crate::{raw_data, Building, BuildingID, FrontPath, Lane};
use abstutil::Timer;
use geom::{Bounds, Distance, HashablePt2D, Line, Polygon};
use std::collections::HashSet;

pub fn make_all_buildings(
    results: &mut Vec<Building>,
    input: &Vec<raw_data::Building>,
    bounds: &Bounds,
    lanes: &Vec<Lane>,
    timer: &mut Timer,
) {
    timer.start("convert buildings");
    let mut center_per_bldg: Vec<HashablePt2D> = Vec::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    timer.start_iter("get building center points", input.len());
    for b in input {
        timer.next();
        let center = b.polygon.center().to_hashable();
        center_per_bldg.push(center);
        query.insert(center);
    }

    // Skip buildings that're too far away from their sidewalk
    let sidewalk_pts = find_sidewalk_points(bounds, query, lanes, Distance::meters(100.0), timer);

    timer.start_iter("create building front paths", center_per_bldg.len());
    for (idx, bldg_center) in center_per_bldg.into_iter().enumerate() {
        timer.next();
        if let Some(sidewalk_pos) = sidewalk_pts.get(&bldg_center) {
            let sidewalk_pt = lanes[sidewalk_pos.lane().0]
                .dist_along(sidewalk_pos.dist_along())
                .0;
            if sidewalk_pt.epsilon_eq(bldg_center.to_pt2d()) {
                timer.warn("Skipping a building because front path has 0 length".to_string());
                continue;
            }
            let polygon = &input[idx].polygon;
            let line = trim_front_path(polygon, Line::new(bldg_center.to_pt2d(), sidewalk_pt));

            let id = BuildingID(results.len());
            results.push(Building {
                id,
                polygon: polygon.clone(),
                osm_tags: input[idx].osm_tags.clone(),
                osm_way_id: input[idx].osm_way_id,
                front_path: FrontPath {
                    sidewalk: *sidewalk_pos,
                    line,
                },
                parking: input[idx].parking.clone(),
                label_center: polygon.polylabel(),
            });
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
fn trim_front_path(poly: &Polygon, path: Line) -> Line {
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

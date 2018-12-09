use crate::make::sidewalk_finder::find_sidewalk_points;
use crate::{raw_data, Building, BuildingID, FrontPath, Lane};
use abstutil::Timer;
use dimensioned::si;
use geom::{Bounds, GPSBounds, HashablePt2D, Line, PolyLine, Pt2D};
use std::collections::HashSet;

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
        let pts = b
            .points
            .iter()
            .map(|coord| Pt2D::from_gps(*coord, gps_bounds).unwrap())
            .collect();
        let center: HashablePt2D = Pt2D::center(&pts).into();
        pts_per_bldg.push(pts);
        center_per_bldg.push(center);
        query.insert(center);
    }

    // Skip buildings that're too far away from their sidewalk
    let sidewalk_pts = find_sidewalk_points(bounds, query, lanes, 100.0 * si::M, timer);

    timer.start_iter("create building front paths", pts_per_bldg.len());
    for (idx, points) in pts_per_bldg.into_iter().enumerate() {
        timer.next();
        let bldg_center = center_per_bldg[idx];
        if let Some(sidewalk_pos) = sidewalk_pts.get(&bldg_center) {
            let sidewalk_pt = lanes[sidewalk_pos.lane().0]
                .dist_along(sidewalk_pos.dist_along())
                .0;
            let line = trim_front_path(&points, Line::new(bldg_center.into(), sidewalk_pt));

            let id = BuildingID(results.len());
            results.push(Building {
                id,
                points,
                osm_tags: input[idx].osm_tags.clone(),
                osm_way_id: input[idx].osm_way_id,
                front_path: FrontPath {
                    bldg: id,
                    sidewalk: *sidewalk_pos,
                    line,
                },
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
    let poly = PolyLine::new(bldg_points.clone());
    if let Some(hit) = poly.intersection(&PolyLine::new(path.points())) {
        Line::new(hit, path.pt2())
    } else {
        // Just give up
        path
    }
}

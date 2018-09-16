use dimensioned::si;
use geom::{Bounds, HashablePt2D, Line, PolyLine, Pt2D};
use geometry;
use make::sidewalk_finder::find_sidewalk_points;
use raw_data;
use std::collections::HashSet;
use {Building, BuildingID, FrontPath, Lane};

pub(crate) fn make_all_buildings(
    results: &mut Vec<Building>,
    input: &Vec<raw_data::Building>,
    bounds: &Bounds,
    lanes: &Vec<Lane>,
) {
    let mut pts_per_bldg: Vec<Vec<Pt2D>> = Vec::new();
    let mut center_per_bldg: Vec<HashablePt2D> = Vec::new();
    let mut query: HashSet<HashablePt2D> = HashSet::new();
    for b in input {
        let pts = b
            .points
            .iter()
            .map(|coord| Pt2D::from_gps(coord, bounds))
            .collect();
        let center: HashablePt2D = geometry::center(&pts).into();
        pts_per_bldg.push(pts);
        center_per_bldg.push(center);
        query.insert(center);
    }

    let sidewalk_pts = find_sidewalk_points(query, lanes);

    for (idx, points) in pts_per_bldg.into_iter().enumerate() {
        let bldg_center = center_per_bldg[idx];
        let (sidewalk, dist_along) = sidewalk_pts[&bldg_center];
        let (sidewalk_pt, _) = lanes[sidewalk.0].dist_along(dist_along);
        let line = trim_front_path(&points, Line::new(bldg_center.into(), sidewalk_pt));

        // Trim buildings that are too far away from their sidewalk
        if line.length() > 100.0 * si::M {
            continue;
        }

        let id = BuildingID(results.len());
        results.push(Building {
            id,
            points,
            osm_tags: input[idx].osm_tags.clone(),
            osm_way_id: input[idx].osm_way_id,
            front_path: FrontPath {
                bldg: id,
                sidewalk: sidewalk,
                line,
                dist_along_sidewalk: dist_along,
            },
        });
    }

    let discarded = input.len() - results.len();
    if discarded > 0 {
        println!(
            "Discarded {} buildings that weren't close enough to a sidewalk",
            discarded
        );
    }
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

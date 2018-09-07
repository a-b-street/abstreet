use geom::{Bounds, Line, PolyLine, Pt2D};
use geometry;
use make::sidewalk_finder::find_sidewalk_points;
use raw_data;
use {Building, BuildingID, FrontPath, Lane};

pub(crate) fn make_building(
    b: &raw_data::Building,
    id: BuildingID,
    bounds: &Bounds,
    lanes: &Vec<Lane>,
) -> Building {
    // TODO consume data, so we dont have to clone tags?
    let points = b.points
        .iter()
        .map(|coord| Pt2D::from_gps(coord, bounds))
        .collect();
    let front_path = find_front_path(id, &points, lanes);

    Building {
        points,
        front_path,
        id,
        osm_way_id: b.osm_way_id,
        osm_tags: b.osm_tags.clone(),
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

fn find_front_path(bldg: BuildingID, bldg_points: &Vec<Pt2D>, lanes: &Vec<Lane>) -> FrontPath {
    let bldg_center = geometry::center(bldg_points);
    let sidewalk_pts = find_sidewalk_points(vec![bldg_center], lanes);
    let (sidewalk, dist_along) = sidewalk_pts.values().next().unwrap();
    let (sidewalk_pt, _) = lanes[sidewalk.0].dist_along(*dist_along);
    let line = trim_front_path(bldg_points, Line::new(bldg_center, sidewalk_pt));

    FrontPath {
        bldg,
        sidewalk: *sidewalk,
        line,
        dist_along_sidewalk: *dist_along,
    }
}

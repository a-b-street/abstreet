use geo;
use geom::{Bounds, Line, Pt2D};
use ordered_float::NotNaN;
use raw_data;
use std::collections::HashMap;
use {Building, BuildingID, LaneType, Road, RoadID};

pub(crate) fn make_building(
    b: &raw_data::Building,
    id: BuildingID,
    bounds: &Bounds,
    roads: &Vec<Road>,
) -> Building {
    // TODO consume data, so we dont have to clone tags?
    let points = b.points
        .iter()
        .map(|coord| Pt2D::from_gps(coord, bounds))
        .collect();
    let front_path = find_front_path(&points, &b.osm_tags, roads);

    Building {
        points,
        front_path,
        id,
        osm_way_id: b.osm_way_id,
        osm_tags: b.osm_tags.clone(),
    }
}

fn find_front_path(
    bldg_points: &Vec<Pt2D>,
    bldg_osm_tags: &HashMap<String, String>,
    roads: &Vec<Road>,
) -> Option<Line> {
    use geo::prelude::{ClosestPoint, EuclideanDistance};

    if let Some(street_name) = bldg_osm_tags.get("addr:street") {
        // TODO start from the side of the building, not the center
        let bldg_center = center(bldg_points);
        let center_pt = geo::Point::new(bldg_center.x(), bldg_center.y());

        // Find all matching sidewalks with that street name, then find the closest point on
        // that sidewalk
        let candidates: Vec<(RoadID, geo::Point<f64>)> = roads
            .iter()
            .filter_map(|r| {
                if r.lane_type == LaneType::Sidewalk && r.osm_tags.get("name") == Some(street_name)
                {
                    if let geo::Closest::SinglePoint(pt) =
                        road_to_line_string(&roads[r.id.0]).closest_point(&center_pt)
                    {
                        return Some((r.id, pt));
                    }
                }
                None
            })
            .collect();

        if let Some(closest) = candidates
            .iter()
            .min_by_key(|pair| NotNaN::new(pair.1.euclidean_distance(&center_pt)).unwrap())
        {
            return Some(Line::new(
                bldg_center,
                Pt2D::new(closest.1.x(), closest.1.y()),
            ));
        }
    }
    None
}

fn center(pts: &Vec<Pt2D>) -> Pt2D {
    let mut x = 0.0;
    let mut y = 0.0;
    for pt in pts {
        x += pt.x();
        y += pt.y();
    }
    let len = pts.len() as f64;
    Pt2D::new(x / len, y / len)
}

fn road_to_line_string(r: &Road) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = r.lane_center_pts
        .points()
        .iter()
        .map(|pt| geo::Point::new(pt.x(), pt.y()))
        .collect();
    pts.into()
}

use geo;
use geom::{Bounds, Line, Pt2D};
use geometry;
use ordered_float::NotNaN;
use raw_data;
use std::collections::BTreeMap;
use {Building, BuildingID, Lane, LaneID, Road};

pub(crate) fn make_building(
    b: &raw_data::Building,
    id: BuildingID,
    bounds: &Bounds,
    lanes: &Vec<Lane>,
    roads: &Vec<Road>,
) -> Building {
    // TODO consume data, so we dont have to clone tags?
    let points = b.points
        .iter()
        .map(|coord| Pt2D::from_gps(coord, bounds))
        .collect();
    let front_path = find_front_path(&points, &b.osm_tags, lanes, roads);

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
    bldg_osm_tags: &BTreeMap<String, String>,
    lanes: &Vec<Lane>,
    roads: &Vec<Road>,
) -> Option<Line> {
    use geo::prelude::{ClosestPoint, EuclideanDistance};

    if let Some(street_name) = bldg_osm_tags.get("addr:street") {
        // TODO start from the side of the building, not the center
        let bldg_center = geometry::center(bldg_points);
        let center_pt = geo::Point::new(bldg_center.x(), bldg_center.y());

        // Find all matching sidewalks with that street name, then find the closest point on
        // that sidewalk
        let candidates: Vec<(LaneID, geo::Point<f64>)> = lanes
            .iter()
            .filter_map(|l| {
                if l.is_sidewalk() && roads[l.parent.0].osm_tags.get("name") == Some(street_name) {
                    if let geo::Closest::SinglePoint(pt) =
                        lane_to_line_string(&lanes[l.id.0]).closest_point(&center_pt)
                    {
                        return Some((l.id, pt));
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

fn lane_to_line_string(l: &Lane) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = l.lane_center_pts
        .points()
        .iter()
        .map(|pt| geo::Point::new(pt.x(), pt.y()))
        .collect();
    pts.into()
}

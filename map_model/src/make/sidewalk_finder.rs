use dimensioned::si;
use geo;
use geo::prelude::{ClosestPoint, EuclideanDistance};
use geom::{HashablePt2D, Pt2D};
use ordered_float::NotNaN;
use std::collections::{HashMap, HashSet};
use {Lane, LaneID};

pub fn find_sidewalk_points(
    pts: HashSet<HashablePt2D>,
    lanes: &Vec<Lane>,
) -> HashMap<HashablePt2D, (LaneID, si::Meter<f64>)> {
    // Get LineStrings of all lanes once.
    let line_strings: Vec<(LaneID, geo::LineString<f64>)> = lanes
        .iter()
        .filter_map(|l| {
            if l.is_sidewalk() {
                Some((l.id, lane_to_line_string(l)))
            } else {
                None
            }
        }).collect();

    // For each point, find the closest point to any sidewalk
    let mut results: HashMap<HashablePt2D, (LaneID, si::Meter<f64>)> = HashMap::new();
    for query_pt in pts {
        let query_geo_pt = geo::Point::new(query_pt.x(), query_pt.y());
        let (sidewalk, raw_pt) = line_strings
            .iter()
            .filter_map(|(id, lines)| {
                if let geo::Closest::SinglePoint(pt) = lines.closest_point(&query_geo_pt) {
                    Some((id, pt))
                } else {
                    None
                }
            }).min_by_key(|(_, pt)| NotNaN::new(pt.euclidean_distance(&query_geo_pt)).unwrap())
            .unwrap();
        let sidewalk_pt = Pt2D::new(raw_pt.x(), raw_pt.y());
        if let Some(dist_along) = lanes[sidewalk.0].dist_along_of_point(sidewalk_pt) {
            results.insert(query_pt.into(), (*sidewalk, dist_along));
        } else {
            panic!("{} isn't on {} according to dist_along_of_point, even though closest_point thinks it is.\n{}", sidewalk_pt, sidewalk, lanes[sidewalk.0].lane_center_pts);
        }
    }
    results
}

fn lane_to_line_string(l: &Lane) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = l
        .lane_center_pts
        .points()
        .iter()
        .map(|pt| geo::Point::new(pt.x(), pt.y()))
        .collect();
    pts.into()
}

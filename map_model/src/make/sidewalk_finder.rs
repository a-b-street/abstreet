use aabb_quadtree::geom::{Point, Rect};
use aabb_quadtree::QuadTree;
use abstutil::Timer;
use dimensioned::si;
use geo;
use geo::prelude::{ClosestPoint, EuclideanDistance};
use geom::{Bounds, HashablePt2D, Pt2D};
use ordered_float::NotNaN;
use std::collections::{HashMap, HashSet};
use {Lane, LaneID};

// If the result doesn't contain a requested point, then there was no matching sidewalk close
// enough.
pub fn find_sidewalk_points(
    bounds: &Bounds,
    pts: HashSet<HashablePt2D>,
    lanes: &Vec<Lane>,
    max_dist_away: si::Meter<f64>,
    timer: &mut Timer,
) -> HashMap<HashablePt2D, (LaneID, si::Meter<f64>)> {
    if pts.is_empty() {
        return HashMap::new();
    }

    // Convert all sidewalks to LineStrings and index them with a quadtree.
    let mut lane_lines_quadtree: QuadTree<usize> = QuadTree::default(bounds.as_bbox());
    let mut lane_lines: Vec<(LaneID, geo::LineString<f64>)> = Vec::new();
    timer.start_iter("lanes to LineStrings", lanes.len());
    for l in lanes {
        timer.next();
        if l.is_sidewalk() {
            lane_lines.push((l.id, lane_to_line_string(l)));
            lane_lines_quadtree.insert_with_box(
                lane_lines.len() - 1,
                l.lane_center_pts.get_bounds().as_bbox(),
            );
        }
    }

    // For each point, find the closest point to any sidewalk, using the quadtree to prune the
    // search.
    let mut results: HashMap<HashablePt2D, (LaneID, si::Meter<f64>)> = HashMap::new();
    timer.start_iter("find closest sidewalk point", pts.len());
    for query_pt in pts {
        timer.next();
        let query_geo_pt = geo::Point::new(query_pt.x(), query_pt.y());
        let query_bbox = Rect {
            top_left: Point {
                x: (query_pt.x() - max_dist_away.value_unsafe) as f32,
                y: (query_pt.y() - max_dist_away.value_unsafe) as f32,
            },
            bottom_right: Point {
                x: (query_pt.x() + max_dist_away.value_unsafe) as f32,
                y: (query_pt.y() + max_dist_away.value_unsafe) as f32,
            },
        };

        if let Some((sidewalk, raw_pt)) = lane_lines_quadtree
            .query(query_bbox)
            .into_iter()
            .filter_map(|(idx, _, _)| {
                let (id, lines) = &lane_lines[*idx];
                if let geo::Closest::SinglePoint(pt) = lines.closest_point(&query_geo_pt) {
                    Some((*id, pt))
                } else {
                    None
                }
            }).min_by_key(|(_, pt)| NotNaN::new(pt.euclidean_distance(&query_geo_pt)).unwrap())
        {
            let sidewalk_pt = Pt2D::new(raw_pt.x(), raw_pt.y());
            if let Some(dist_along) = lanes[sidewalk.0].dist_along_of_point(sidewalk_pt) {
                results.insert(query_pt.into(), (sidewalk, dist_along));
            } else {
                panic!("{} isn't on {} according to dist_along_of_point, even though closest_point thinks it is.\n{}", sidewalk_pt, sidewalk, lanes[sidewalk.0].lane_center_pts);
            }
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

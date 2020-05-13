use crate::{Bounds, Distance, Pt2D};
use aabb_quadtree::geom::{Point, Rect};
use aabb_quadtree::QuadTree;
use geo;
use geo::prelude::{ClosestPoint, EuclideanDistance};
use std::collections::HashMap;

pub struct FindClosest<K> {
    // TODO maybe any type of geo:: thing
    geometries: HashMap<K, geo::LineString<f64>>,
    quadtree: QuadTree<K>,
}

impl<K> FindClosest<K>
where
    K: Clone + std::cmp::Eq + std::hash::Hash + std::fmt::Debug,
{
    pub fn new(bounds: &Bounds) -> FindClosest<K> {
        FindClosest {
            geometries: HashMap::new(),
            quadtree: QuadTree::default(bounds.as_bbox()),
        }
    }

    pub fn add(&mut self, key: K, pts: &Vec<Pt2D>) {
        self.geometries.insert(key.clone(), pts_to_line_string(pts));
        self.quadtree
            .insert_with_box(key, Bounds::from(pts).as_bbox());
    }

    pub fn all_close_pts(
        &self,
        query_pt: Pt2D,
        max_dist_away: Distance,
    ) -> Vec<(K, Pt2D, Distance)> {
        let query_geom = geo::Point::new(query_pt.x(), query_pt.y());
        let query_bbox = Rect {
            top_left: Point {
                x: query_pt.x() - max_dist_away.inner_meters(),
                y: query_pt.y() - max_dist_away.inner_meters(),
            },
            bottom_right: Point {
                x: query_pt.x() + max_dist_away.inner_meters(),
                y: query_pt.y() + max_dist_away.inner_meters(),
            },
        };

        self.quadtree
            .query(query_bbox)
            .into_iter()
            .filter_map(|(key, _, _)| {
                if let geo::Closest::SinglePoint(pt) =
                    self.geometries[&key].closest_point(&query_geom)
                {
                    let dist = Distance::meters(pt.euclidean_distance(&query_geom));
                    if dist <= max_dist_away {
                        Some((key.clone(), Pt2D::new(pt.x(), pt.y()), dist))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    // Finds the closest point on the existing geometry to the query pt.
    pub fn closest_pt(&self, query_pt: Pt2D, max_dist_away: Distance) -> Option<(K, Pt2D)> {
        self.all_close_pts(query_pt, max_dist_away)
            .into_iter()
            .min_by_key(|(_, _, dist)| *dist)
            .map(|(k, pt, _)| (k, pt))
    }
}

fn pts_to_line_string(raw_pts: &Vec<Pt2D>) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = raw_pts
        .iter()
        .map(|pt| geo::Point::new(pt.x(), pt.y()))
        .collect();
    pts.into()
}

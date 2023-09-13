use std::collections::BTreeMap;

use geo::{ClosestPoint, Contains, EuclideanDistance, Intersects};

use crate::conversions::pts_to_line_string;
use crate::{Bounds, Distance, Polygon, Pt2D, QuadTree};

// TODO Maybe use https://crates.io/crates/spatial-join proximity maps

/// A quad-tree to quickly find the closest points to some polylines.
#[derive(Clone)]
pub struct FindClosest<K> {
    // TODO maybe any type of geo:: thing
    geometries: BTreeMap<K, geo::LineString>,
    quadtree: QuadTree<K>,
}

impl<K> FindClosest<K>
where
    K: Clone + Ord + std::fmt::Debug,
{
    pub fn new() -> FindClosest<K> {
        FindClosest {
            geometries: BTreeMap::new(),
            quadtree: QuadTree::new(),
        }
    }

    /// Add an object to the quadtree, remembering some key associated with the points.
    /// TODO This doesn't properly handle single points, and will silently fail by never returning
    /// any matches.
    pub fn add(&mut self, key: K, pts: &[Pt2D]) {
        self.geometries.insert(key.clone(), pts_to_line_string(pts));
        self.quadtree.insert_with_box(key, Bounds::from(pts));
    }

    /// Adds the outer ring of a polygon to the quadtree.
    pub fn add_polygon(&mut self, key: K, polygon: &Polygon) {
        self.add(key, polygon.get_outer_ring().points());
    }

    /// For every object within some distance of a query point, return the (object's key, point on
    /// the object's polyline, distance away).
    pub fn all_close_pts(
        &self,
        query_pt: Pt2D,
        max_dist_away: Distance,
    ) -> Vec<(K, Pt2D, Distance)> {
        let query_geom = geo::Point::new(query_pt.x(), query_pt.y());

        self.quadtree
            .query_bbox_borrow(
                Polygon::rectangle_centered(query_pt, max_dist_away, max_dist_away).get_bounds(),
            )
            .filter_map(|key| {
                if let geo::Closest::SinglePoint(pt) =
                    self.geometries[key].closest_point(&query_geom)
                {
                    let dist = Distance::meters(pt.euclidean_distance(&query_geom));
                    if dist <= max_dist_away {
                        Some((key.clone(), Pt2D::new(pt.x(), pt.y()), dist))
                    } else {
                        None
                    }
                } else if self.geometries[key].contains(&query_geom) {
                    // TODO Yay, FindClosest has a bug. :P
                    Some((key.clone(), query_pt, Distance::ZERO))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Finds the closest point on the existing geometry to the query pt.
    pub fn closest_pt(&self, query_pt: Pt2D, max_dist_away: Distance) -> Option<(K, Pt2D)> {
        self.all_close_pts(query_pt, max_dist_away)
            .into_iter()
            .min_by_key(|(_, _, dist)| *dist)
            .map(|(k, pt, _)| (k, pt))
    }

    /// Find all objects with a point inside the query polygon
    pub fn all_points_inside(&self, query: &Polygon) -> Vec<K> {
        let query_geo: geo::Polygon = query.clone().into();

        self.quadtree
            .query_bbox_borrow(query.get_bounds())
            .filter_map(|key| {
                if self.geometries[key].intersects(&query_geo) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

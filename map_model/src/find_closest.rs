use aabb_quadtree::geom::{Point, Rect};
use aabb_quadtree::QuadTree;
use dimensioned::si;
use geo;
use geo::prelude::{ClosestPoint, EuclideanDistance};
use geom::{Bounds, PolyLine, Pt2D};
use ordered_float::NotNaN;
use std::collections::HashMap;

// TODO Refactor and generalize all of this...

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

    pub fn add(&mut self, key: K, pts: &PolyLine) {
        self.geometries.insert(key.clone(), pts_to_line_string(pts));
        self.quadtree
            .insert_with_box(key, pts.get_bounds().as_bbox());
    }

    // Finds the closest point on the existing geometry to the query pt.
    pub fn closest_pt(&self, query_pt: Pt2D, max_dist_away: si::Meter<f64>) -> Option<(K, Pt2D)> {
        let query_geom = geo::Point::new(query_pt.x(), query_pt.y());
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

        self.quadtree
            .query(query_bbox)
            .into_iter()
            .filter_map(|(key, _, _)| {
                if let geo::Closest::SinglePoint(pt) =
                    self.geometries[&key].closest_point(&query_geom)
                {
                    let dist = pt.euclidean_distance(&query_geom);
                    if dist * si::M <= max_dist_away {
                        Some((key, pt, NotNaN::new(dist).unwrap()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }).min_by_key(|(_, _, dist)| *dist)
            .map(|(key, pt, _)| (key.clone(), Pt2D::new(pt.x(), pt.y())))
    }

    /*pub fn match_pts(&self, pts: &PolyLine, padding: f64) -> Option<K> {
        let query_geom = pts_to_line_string(pts);
        let mut query_bbox = pts.get_bounds().as_bbox();
        let p = padding as f32;
        query_bbox.top_left.x -= p;
        query_bbox.top_left.y -= p;
        query_bbox.bottom_right.x += p;
        query_bbox.bottom_right.y += p;

        self.quadtree
            .query(query_bbox)
            .into_iter()
            .min_by_key(|(key, _, _)| {
                NotNaN::new(query_geom.euclidean_distance(&self.geometries[key])).unwrap()
            }).map(|(key, _, _)| key.clone())
    }*/
}

fn pts_to_line_string(pts: &PolyLine) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = pts
        .points()
        .iter()
        .map(|pt| geo::Point::new(pt.x(), pt.y()))
        .collect();
    pts.into()
}

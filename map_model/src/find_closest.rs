use aabb_quadtree::QuadTree;
use geo;
use geo::prelude::EuclideanDistance;
use geom::{Bounds, PolyLine};
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

    pub fn match_pts(&self, pts: &PolyLine) -> Option<K> {
        let query_geom = pts_to_line_string(pts);
        let query_bbox = pts.get_bounds().as_bbox();

        self.quadtree
            .query(query_bbox)
            .into_iter()
            .min_by_key(|(key, _, _)| {
                NotNaN::new(query_geom.euclidean_distance(&self.geometries[key])).unwrap()
            }).map(|(key, _, _)| key.clone())
    }
}

fn pts_to_line_string(pts: &PolyLine) -> geo::LineString<f64> {
    let pts: Vec<geo::Point<f64>> = pts
        .points()
        .iter()
        .map(|pt| geo::Point::new(pt.x(), pt.y()))
        .collect();
    pts.into()
}

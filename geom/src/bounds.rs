use serde::{Deserialize, Serialize};

use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{RTree, SelectionFunction, AABB};

use crate::{Circle, Distance, LonLat, Polygon, Pt2D, Ring};

/// Represents a rectangular boundary of `Pt2D` points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    /// A boundary including no points.
    pub fn new() -> Bounds {
        Bounds {
            min_x: f64::MAX,
            min_y: f64::MAX,
            max_x: f64::MIN,
            max_y: f64::MIN,
        }
    }

    pub fn zero() -> Self {
        Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        }
    }

    /// Create a boundary covering some points.
    pub fn from(pts: &[Pt2D]) -> Bounds {
        let mut b = Bounds::new();
        for pt in pts {
            b.update(*pt);
        }
        b
    }

    /// Create a boundary covering some polygons.
    pub fn from_polygons(polygons: &[Polygon]) -> Bounds {
        let mut b = Bounds::new();
        for poly in polygons {
            for ring in poly.get_rings() {
                for pt in ring.points() {
                    b.update(*pt);
                }
            }
        }
        b
    }

    /// Update the boundary to include this point.
    pub fn update(&mut self, pt: Pt2D) {
        self.min_x = self.min_x.min(pt.x());
        self.max_x = self.max_x.max(pt.x());
        self.min_y = self.min_y.min(pt.y());
        self.max_y = self.max_y.max(pt.y());
    }

    /// Unions two boundaries.
    pub fn union(&mut self, other: Bounds) {
        self.update(Pt2D::new(other.min_x, other.min_y));
        self.update(Pt2D::new(other.max_x, other.max_y));
    }

    /// Expand the existing boundary by some distance evenly on all sides.
    pub fn add_buffer(&mut self, sides: Distance) {
        self.min_x -= sides.inner_meters();
        self.max_x += sides.inner_meters();
        self.min_y -= sides.inner_meters();
        self.max_y += sides.inner_meters();
    }

    /// Transform the boundary by scaling its corners.
    pub fn scale(mut self, factor: f64) -> Self {
        self.min_x *= factor;
        self.min_y *= factor;
        self.max_x *= factor;
        self.max_y *= factor;
        self
    }

    /// True if the point is within the boundary.
    pub fn contains(&self, pt: Pt2D) -> bool {
        pt.x() >= self.min_x && pt.x() <= self.max_x && pt.y() >= self.min_y && pt.y() <= self.max_y
    }

    /// Converts the boundary to the format used by `rstar`.
    pub fn as_bbox<T>(&self, data: T) -> GeomWithData<Rectangle<[f64; 2]>, T> {
        GeomWithData::new(
            Rectangle::from_corners([self.min_x, self.min_y], [self.max_x, self.max_y]),
            data,
        )
    }

    /// Creates a rectangle covering this boundary.
    pub fn get_rectangle(&self) -> Polygon {
        Ring::must_new(vec![
            Pt2D::new(self.min_x, self.min_y),
            Pt2D::new(self.max_x, self.min_y),
            Pt2D::new(self.max_x, self.max_y),
            Pt2D::new(self.min_x, self.max_y),
            Pt2D::new(self.min_x, self.min_y),
        ])
        .into_polygon()
    }

    /// Creates a circle centered in the middle of this boundary. Always uses the half width as a
    /// radius, so if the width and height don't match, this is pretty meaningless.
    pub fn to_circle(&self) -> Circle {
        Circle::new(self.center(), Distance::meters(self.width() / 2.0))
    }

    /// The width of this boundary.
    // TODO Really should be Distance
    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    /// The height of this boundary.
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }

    /// The center point of this boundary.
    pub fn center(&self) -> Pt2D {
        Pt2D::new(
            self.min_x + self.width() / 2.0,
            self.min_y + self.height() / 2.0,
        )
    }
}

/// Represents a rectangular boundary of `LonLat` points. After building one of these, `LonLat`s
/// can be transformed into `Pt2D`s, treating the top-left of the boundary as (0, 0), and growing
/// to the right and down (screen-drawing order, not Cartesian) in meters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GPSBounds {
    pub min_lon: f64,
    pub min_lat: f64,
    pub max_lon: f64,
    pub max_lat: f64,
}

impl GPSBounds {
    /// A boundary including no points.
    pub fn new() -> GPSBounds {
        GPSBounds {
            min_lon: f64::MAX,
            min_lat: f64::MAX,
            max_lon: f64::MIN,
            max_lat: f64::MIN,
        }
    }

    /// Create a boundary covering some points.
    pub fn from(pts: Vec<LonLat>) -> GPSBounds {
        let mut b = GPSBounds::new();
        for pt in pts {
            b.update(pt);
        }
        b
    }

    /// Update the boundary to include this point.
    pub fn update(&mut self, pt: LonLat) {
        self.min_lon = self.min_lon.min(pt.x());
        self.max_lon = self.max_lon.max(pt.x());
        self.min_lat = self.min_lat.min(pt.y());
        self.max_lat = self.max_lat.max(pt.y());
    }

    /// True if the point is within the boundary.
    pub fn contains(&self, pt: LonLat) -> bool {
        pt.x() >= self.min_lon
            && pt.x() <= self.max_lon
            && pt.y() >= self.min_lat
            && pt.y() <= self.max_lat
    }

    /// The bottom-right corner of the boundary, in map-space.
    // TODO cache this
    pub fn get_max_world_pt(&self) -> Pt2D {
        let width = LonLat::new(self.min_lon, self.min_lat)
            .gps_dist(LonLat::new(self.max_lon, self.min_lat));
        let height = LonLat::new(self.min_lon, self.min_lat)
            .gps_dist(LonLat::new(self.min_lon, self.max_lat));
        Pt2D::new(width.inner_meters(), height.inner_meters())
    }

    /// Converts the boundary to map-space.
    pub fn to_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        b.update(Pt2D::new(0.0, 0.0));
        b.update(self.get_max_world_pt());
        b
    }

    /// Convert all points to map-space, failing if any points are outside this boundary.
    pub fn try_convert(&self, pts: &[LonLat]) -> Option<Vec<Pt2D>> {
        let mut result = Vec::new();
        for gps in pts {
            if !self.contains(*gps) {
                return None;
            }
            result.push(gps.to_pt(self));
        }
        Some(result)
    }

    /// Convert all points to map-space. The points may be outside this boundary.
    pub fn convert(&self, pts: &[LonLat]) -> Vec<Pt2D> {
        pts.iter().map(|gps| gps.to_pt(self)).collect()
    }

    /// Convert map-space points back to `LonLat`s. This is only valid if the `GPSBounds` used
    /// is the same as the one used to originally produce the `Pt2D`s.
    pub fn convert_back(&self, pts: &[Pt2D]) -> Vec<LonLat> {
        pts.iter().map(|pt| pt.to_gps(self)).collect()
    }

    pub fn convert_back_xy(&self, x: f64, y: f64) -> LonLat {
        let (width, height) = {
            let pt = self.get_max_world_pt();
            (pt.x(), pt.y())
        };

        let lon = (x / width * (self.max_lon - self.min_lon)) + self.min_lon;
        let lat = self.min_lat + ((self.max_lat - self.min_lat) * (height - y) / height);
        LonLat::new(lon, lat)
    }

    /// Returns points in order covering this boundary.
    pub fn get_rectangle(&self) -> Vec<LonLat> {
        vec![
            LonLat::new(self.min_lon, self.min_lat),
            LonLat::new(self.max_lon, self.min_lat),
            LonLat::new(self.max_lon, self.max_lat),
            LonLat::new(self.min_lon, self.max_lat),
            LonLat::new(self.min_lon, self.min_lat),
        ]
    }
}

// Convenient wrapper around rstar
#[derive(Clone)]
pub struct QuadTree<T>(RTree<GeomWithData<Rectangle<[f64; 2]>, T>>);

impl<T> QuadTree<T> {
    pub fn new() -> Self {
        Self(RTree::new())
    }

    pub fn builder() -> QuadTreeBuilder<T> {
        QuadTreeBuilder::new()
    }

    /// Slow, prefer bulk_load or QuadTreeBuilder
    pub fn insert(&mut self, entry: GeomWithData<Rectangle<[f64; 2]>, T>) {
        self.0.insert(entry);
    }
    pub fn insert_with_box(&mut self, data: T, bbox: Bounds) {
        self.0.insert(bbox.as_bbox(data));
    }

    pub fn bulk_load(entries: Vec<GeomWithData<Rectangle<[f64; 2]>, T>>) -> Self {
        Self(RTree::bulk_load(entries))
    }

    pub fn query_bbox_borrow(&self, bbox: Bounds) -> impl Iterator<Item = &T> + '_ {
        let envelope = AABB::from_corners([bbox.min_x, bbox.min_y], [bbox.max_x, bbox.max_y]);
        self.0
            .locate_in_envelope_intersecting(&envelope)
            .map(|x| &x.data)
    }
}

impl<T: Copy> QuadTree<T> {
    pub fn query_bbox(&self, bbox: Bounds) -> impl Iterator<Item = T> + '_ {
        let envelope = AABB::from_corners([bbox.min_x, bbox.min_y], [bbox.max_x, bbox.max_y]);
        self.0
            .locate_in_envelope_intersecting(&envelope)
            .map(|x| x.data)
    }
}

impl<T: PartialEq> QuadTree<T> {
    // TODO Inefficient
    pub fn remove(&mut self, data: T) -> Option<T> {
        self.0
            .remove_with_selection_function(Selector(data))
            .map(|item| item.data)
    }
}

struct Selector<T>(T);

impl<T: PartialEq> SelectionFunction<GeomWithData<Rectangle<[f64; 2]>, T>> for Selector<T> {
    fn should_unpack_parent(&self, _envelope: &AABB<[f64; 2]>) -> bool {
        // TODO Maybe we can ask the caller to estimate where the previous object was?
        true
    }

    fn should_unpack_leaf(&self, leaf: &GeomWithData<Rectangle<[f64; 2]>, T>) -> bool {
        self.0 == leaf.data
    }
}

pub struct QuadTreeBuilder<T>(Vec<GeomWithData<Rectangle<[f64; 2]>, T>>);

impl<T> QuadTreeBuilder<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add(&mut self, entry: GeomWithData<Rectangle<[f64; 2]>, T>) {
        self.0.push(entry);
    }
    pub fn add_with_box(&mut self, data: T, bbox: Bounds) {
        self.0.push(bbox.as_bbox(data));
    }

    pub fn build(self) -> QuadTree<T> {
        QuadTree::bulk_load(self.0)
    }
}

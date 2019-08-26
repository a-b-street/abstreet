use crate::{LonLat, Pt2D};
use aabb_quadtree::geom::{Point, Rect};
use serde_derive::{Deserialize, Serialize};
use std::f64;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn new() -> Bounds {
        Bounds {
            min_x: f64::MAX,
            min_y: f64::MAX,
            max_x: f64::MIN,
            max_y: f64::MIN,
        }
    }

    pub fn from(pts: &Vec<Pt2D>) -> Bounds {
        let mut b = Bounds::new();
        for pt in pts {
            b.update(*pt);
        }
        b
    }

    pub fn update(&mut self, pt: Pt2D) {
        self.min_x = self.min_x.min(pt.x());
        self.max_x = self.max_x.max(pt.x());
        self.min_y = self.min_y.min(pt.y());
        self.max_y = self.max_y.max(pt.y());
    }

    pub fn union(&mut self, other: Bounds) {
        self.update(Pt2D::new(other.min_x, other.min_y));
        self.update(Pt2D::new(other.max_x, other.max_y));
    }

    pub fn contains(&self, pt: Pt2D) -> bool {
        pt.x() >= self.min_x && pt.x() <= self.max_x && pt.y() >= self.min_y && pt.y() <= self.max_y
    }

    pub fn as_bbox(&self) -> Rect {
        Rect {
            top_left: Point {
                x: self.min_x as f32,
                y: self.min_y as f32,
            },
            bottom_right: Point {
                x: self.max_x as f32,
                y: self.max_y as f32,
            },
        }
    }

    pub fn get_corners(&self) -> Vec<Pt2D> {
        vec![
            Pt2D::new(self.min_x, self.min_y),
            Pt2D::new(self.max_x, self.min_y),
            Pt2D::new(self.max_x, self.max_y),
            Pt2D::new(self.min_x, self.max_y),
        ]
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GPSBounds {
    pub(crate) min_lon: f64,
    pub(crate) min_lat: f64,
    pub(crate) max_lon: f64,
    pub(crate) max_lat: f64,

    // TODO hack to easily construct test maps
    pub represents_world_space: bool,
}

impl GPSBounds {
    pub fn new() -> GPSBounds {
        GPSBounds {
            min_lon: f64::MAX,
            min_lat: f64::MAX,
            max_lon: f64::MIN,
            max_lat: f64::MIN,
            represents_world_space: false,
        }
    }

    pub fn from(pts: &Vec<LonLat>) -> GPSBounds {
        let mut b = GPSBounds::new();
        for pt in pts {
            b.update(*pt);
        }
        b
    }

    pub fn update(&mut self, pt: LonLat) {
        self.min_lon = self.min_lon.min(pt.longitude);
        self.max_lon = self.max_lon.max(pt.longitude);
        self.min_lat = self.min_lat.min(pt.latitude);
        self.max_lat = self.max_lat.max(pt.latitude);
    }

    pub fn contains(&self, pt: LonLat) -> bool {
        pt.longitude >= self.min_lon
            && pt.longitude <= self.max_lon
            && pt.latitude >= self.min_lat
            && pt.latitude <= self.max_lat
    }

    pub fn as_bbox(&self) -> Rect {
        Rect {
            top_left: Point {
                x: self.min_lon as f32,
                y: self.min_lat as f32,
            },
            bottom_right: Point {
                x: self.max_lon as f32,
                y: self.max_lat as f32,
            },
        }
    }

    pub fn get_corners(&self) -> Vec<LonLat> {
        vec![
            LonLat::new(self.min_lon, self.min_lat),
            LonLat::new(self.max_lon, self.min_lat),
            LonLat::new(self.max_lon, self.max_lat),
            LonLat::new(self.min_lon, self.max_lat),
        ]
    }

    // TODO cache this
    pub fn get_max_world_pt(&self) -> Pt2D {
        let width = LonLat::new(self.min_lon, self.min_lat)
            .gps_dist_meters(LonLat::new(self.max_lon, self.min_lat));
        let height = LonLat::new(self.min_lon, self.min_lat)
            .gps_dist_meters(LonLat::new(self.min_lon, self.max_lat));
        Pt2D::new(width.inner_meters(), height.inner_meters())
    }

    pub fn to_bounds(&self) -> Bounds {
        let mut b = Bounds::new();
        b.update(Pt2D::new(0.0, 0.0));
        b.update(self.get_max_world_pt());
        b
    }

    pub fn try_convert(&self, pts: &Vec<LonLat>) -> Option<Vec<Pt2D>> {
        let mut result = Vec::new();
        for pt in pts {
            result.push(Pt2D::from_gps(*pt, self)?);
        }
        Some(result)
    }

    // Results can be out-of-bounds.
    pub fn forcibly_convert(&self, pts: &Vec<LonLat>) -> Vec<Pt2D> {
        pts.iter()
            .map(|pt| Pt2D::forcibly_from_gps(*pt, self))
            .collect()
    }

    pub fn must_convert(&self, pts: &Vec<LonLat>) -> Vec<Pt2D> {
        self.try_convert(pts).unwrap()
    }

    pub fn must_convert_back(&self, pts: &Vec<Pt2D>) -> Vec<LonLat> {
        pts.iter().map(|pt| pt.to_gps(self).unwrap()).collect()
    }

    // TODO don't hardcode
    pub fn seattle_bounds() -> GPSBounds {
        let mut b = GPSBounds::new();
        b.update(LonLat::new(-122.4416, 47.5793));
        b.update(LonLat::new(-122.2421, 47.7155));
        b
    }
}

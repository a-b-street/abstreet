use crate::{Angle, GPSBounds, LonLat};
use aabb_quadtree::geom::{Point, Rect};
use ordered_float::NotNaN;
use serde_derive::{Deserialize, Serialize};
use std::f64;
use std::fmt;

// This represents world-space in meters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pt2D {
    x: f64,
    y: f64,
}

impl Pt2D {
    pub fn new(x: f64, y: f64) -> Pt2D {
        if !x.is_finite() || !y.is_finite() {
            panic!("Bad Pt2D {}, {}", x, y);
        }

        // TODO enforce after fixing:
        // - shift_polyline goes OOB sometimes
        // - convert_map uses this for GPS I think?
        if x < 0.0 || y < 0.0 {
            //println!("Bad pt: {}, {}", x, y);
        }
        //assert!(x >= 0.0);
        //assert!(y >= 0.0);

        Pt2D { x, y }
    }

    pub fn from_gps(gps: LonLat, b: &GPSBounds) -> Option<Pt2D> {
        // TODO hack to construct test maps more easily
        if b.represents_world_space {
            return Some(Pt2D::new(gps.longitude, gps.latitude));
        }

        if !b.contains(gps) {
            return None;
        }

        let (width, height) = {
            let pt = b.get_max_world_pt();
            (pt.x, pt.y)
        };

        let x = (gps.longitude - b.min_lon) / (b.max_lon - b.min_lon) * width;
        // Invert y, so that the northernmost latitude is 0. Screen drawing order, not Cartesian grid.
        let y = height - ((gps.latitude - b.min_lat) / (b.max_lat - b.min_lat) * height);
        Some(Pt2D::new(x, y))
    }

    pub fn to_gps(&self, b: &GPSBounds) -> Option<LonLat> {
        let (width, height) = {
            let pt = b.get_max_world_pt();
            (pt.x, pt.y)
        };
        if self.x < 0.0 || self.y < 0.0 || self.x > width || self.y > height {
            return None;
        }

        let lon = (self.x / width * (b.max_lon - b.min_lon)) + b.min_lon;
        let lat = b.min_lat + ((b.max_lat - b.min_lat) * (height - self.y) / height);

        Some(LonLat::new(lon, lat))
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    // TODO better name
    // TODO Meters for dist?
    pub fn project_away(&self, dist: f64, theta: Angle) -> Pt2D {
        // If negative, caller should use theta.opposite()
        assert!(dist >= 0.0);

        let (sin, cos) = theta.normalized_radians().sin_cos();
        Pt2D::new(self.x() + dist * cos, self.y() + dist * sin)
    }

    pub fn angle_to(&self, to: Pt2D) -> Angle {
        // DON'T invert y here
        Angle::new((to.y() - self.y()).atan2(to.x() - self.x()))
    }

    pub fn offset(&self, dx: f64, dy: f64) -> Pt2D {
        Pt2D::new(self.x() + dx, self.y() + dy)
    }

    pub fn center(pts: &Vec<Pt2D>) -> Pt2D {
        let mut x = 0.0;
        let mut y = 0.0;
        for pt in pts {
            x += pt.x();
            y += pt.y();
        }
        let len = pts.len() as f64;
        Pt2D::new(x / len, y / len)
    }
}

impl fmt::Display for Pt2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pt2D({0}, {1})", self.x(), self.y())
    }
}

impl From<HashablePt2D> for Pt2D {
    fn from(pt: HashablePt2D) -> Self {
        Pt2D::new(pt.x(), pt.y())
    }
}

// This isn't opinionated about what the (x, y) represents -- could be lat/lon or world space.
// TODO So rename it HashablePair or something
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct HashablePt2D {
    x_nan: NotNaN<f64>,
    y_nan: NotNaN<f64>,
}

impl HashablePt2D {
    pub fn new(x: f64, y: f64) -> HashablePt2D {
        HashablePt2D {
            x_nan: NotNaN::new(x).unwrap(),
            y_nan: NotNaN::new(y).unwrap(),
        }
    }

    pub fn x(&self) -> f64 {
        self.x_nan.into_inner()
    }

    pub fn y(&self) -> f64 {
        self.y_nan.into_inner()
    }
}

impl From<Pt2D> for HashablePt2D {
    fn from(pt: Pt2D) -> Self {
        HashablePt2D::new(pt.x(), pt.y())
    }
}

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

    pub fn update(&mut self, pt: Pt2D) {
        self.min_x = self.min_x.min(pt.x);
        self.max_x = self.max_x.max(pt.x);
        self.min_y = self.min_y.min(pt.y);
        self.max_y = self.max_y.max(pt.y);
    }

    pub fn contains(&self, pt: Pt2D) -> bool {
        pt.x >= self.min_x && pt.x <= self.max_x && pt.y >= self.min_y && pt.y <= self.max_y
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
}

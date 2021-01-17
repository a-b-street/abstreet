use std::fmt;

use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::{trim_f64, Angle, Distance, GPSBounds, LonLat, EPSILON_DIST};

/// This represents world-space in meters.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Pt2D {
    x: f64,
    y: f64,
}

impl std::cmp::PartialEq for Pt2D {
    fn eq(&self, other: &Pt2D) -> bool {
        self.approx_eq(*other, EPSILON_DIST)
    }
}

impl Pt2D {
    pub fn new(x: f64, y: f64) -> Pt2D {
        if !x.is_finite() || !y.is_finite() {
            panic!("Bad Pt2D {}, {}", x, y);
        }

        // TODO enforce >=0

        Pt2D {
            x: trim_f64(x),
            y: trim_f64(y),
        }
    }

    // TODO This is a small first step...
    pub fn approx_eq(self, other: Pt2D, threshold: Distance) -> bool {
        self.dist_to(other) <= threshold
    }

    /// Can go out of bounds.
    pub fn to_gps(self, b: &GPSBounds) -> LonLat {
        let (width, height) = {
            let pt = b.get_max_world_pt();
            (pt.x(), pt.y())
        };
        let lon = (self.x() / width * (b.max_lon - b.min_lon)) + b.min_lon;
        let lat = b.min_lat + ((b.max_lat - b.min_lat) * (height - self.y()) / height);
        LonLat::new(lon, lat)
    }

    pub fn x(self) -> f64 {
        self.x
    }

    pub fn y(self) -> f64 {
        self.y
    }

    // TODO better name
    pub fn project_away(self, dist: Distance, theta: Angle) -> Pt2D {
        // If negative, caller should use theta.opposite()
        assert!(dist >= Distance::ZERO);

        let (sin, cos) = theta.normalized_radians().sin_cos();
        Pt2D::new(
            self.x() + dist.inner_meters() * cos,
            self.y() + dist.inner_meters() * sin,
        )
    }

    // TODO valid to do euclidean distance on world-space points that're formed from
    // Haversine?
    pub(crate) fn raw_dist_to(self, to: Pt2D) -> f64 {
        ((self.x() - to.x()).powi(2) + (self.y() - to.y()).powi(2)).sqrt()
    }

    pub fn dist_to(self, to: Pt2D) -> Distance {
        Distance::meters(self.raw_dist_to(to))
    }

    /// Pretty meaningless units, for comparing distances very roughly
    pub fn fast_dist(self, other: Pt2D) -> NotNan<f64> {
        NotNan::new((self.x() - other.x()).powi(2) + (self.y() - other.y()).powi(2)).unwrap()
    }

    pub fn angle_to(self, to: Pt2D) -> Angle {
        // DON'T invert y here
        Angle::new_rads((to.y() - self.y()).atan2(to.x() - self.x()))
    }

    pub fn offset(self, dx: f64, dy: f64) -> Pt2D {
        Pt2D::new(self.x() + dx, self.y() + dy)
    }

    pub fn center(pts: &Vec<Pt2D>) -> Pt2D {
        if pts.is_empty() {
            panic!("Can't find center of 0 points");
        }
        let mut x = 0.0;
        let mut y = 0.0;
        for pt in pts {
            x += pt.x();
            y += pt.y();
        }
        let len = pts.len() as f64;
        Pt2D::new(x / len, y / len)
    }

    // Temporary until Pt2D has proper resolution.
    pub fn approx_dedupe(pts: Vec<Pt2D>, threshold: Distance) -> Vec<Pt2D> {
        // Just use dedup() on the Vec.
        assert_ne!(threshold, EPSILON_DIST);
        let mut result: Vec<Pt2D> = Vec::new();
        for pt in pts {
            if result.is_empty() || !result.last().unwrap().approx_eq(pt, threshold) {
                result.push(pt);
            }
        }
        result
    }

    // TODO Try to deprecate in favor of Ring::get_shorter_slice_btwn
    pub fn find_pts_between(
        pts: &Vec<Pt2D>,
        start: Pt2D,
        end: Pt2D,
        threshold: Distance,
    ) -> Option<Vec<Pt2D>> {
        let mut result = Vec::new();
        for pt in pts {
            if result.is_empty() && pt.approx_eq(start, threshold) {
                result.push(*pt);
            } else if !result.is_empty() {
                result.push(*pt);
            }
            // start and end might be the same.
            if !result.is_empty() && pt.approx_eq(end, threshold) {
                return Some(result);
            }
        }

        // start wasn't in the list!
        if result.is_empty() {
            return None;
        }

        // Go through again, looking for end
        for pt in pts {
            result.push(*pt);
            if pt.approx_eq(end, threshold) {
                return Some(result);
            }
        }
        // Didn't find end
        None
    }

    pub fn to_hashable(self) -> HashablePt2D {
        HashablePt2D {
            x_nan: NotNan::new(self.x()).unwrap(),
            y_nan: NotNan::new(self.y()).unwrap(),
        }
    }
}

impl fmt::Display for Pt2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pt2D({0}, {1})", self.x(), self.y())
    }
}

/// This represents world space, NOT LonLat.
// TODO So rename it HashablePair or something
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct HashablePt2D {
    x_nan: NotNan<f64>,
    y_nan: NotNan<f64>,
}

impl HashablePt2D {
    pub fn to_pt2d(self) -> Pt2D {
        Pt2D::new(self.x_nan.into_inner(), self.y_nan.into_inner())
    }
}

impl From<Pt2D> for geo::Coordinate<f64> {
    fn from(pt: Pt2D) -> Self {
        geo::Coordinate { x: pt.x, y: pt.y }
    }
}

impl From<Pt2D> for geo::Point<f64> {
    fn from(pt: Pt2D) -> Self {
        geo::Point::new(pt.x, pt.y)
    }
}

impl From<geo::Coordinate<f64>> for Pt2D {
    fn from(coord: geo::Coordinate<f64>) -> Self {
        Pt2D::new(coord.x, coord.y)
    }
}

impl From<geo::Point<f64>> for Pt2D {
    fn from(point: geo::Point<f64>) -> Self {
        Pt2D::new(point.x(), point.y())
    }
}

use std::fmt;

use geo::Simplify;
use ordered_float::NotNan;
use serde::{Deserialize, Serialize};

use crate::conversions::pts_to_line_string;
use crate::{
    deserialize_f64, serialize_f64, trim_f64, Angle, Distance, GPSBounds, LonLat, EPSILON_DIST,
};

/// This represents world-space in meters.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Pt2D {
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")]
    x: f64,
    #[serde(serialize_with = "serialize_f64", deserialize_with = "deserialize_f64")]
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

    pub fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    // TODO This is a small first step...
    pub fn approx_eq(self, other: Pt2D, threshold: Distance) -> bool {
        self.dist_to(other) <= threshold
    }

    /// Can go out of bounds.
    pub fn to_gps(self, b: &GPSBounds) -> LonLat {
        b.convert_back_xy(self.x(), self.y())
    }

    pub fn x(self) -> f64 {
        self.x
    }

    pub fn y(self) -> f64 {
        self.y
    }

    /// If distance is negative, this projects a point in theta.opposite()
    pub fn project_away(self, dist: Distance, theta: Angle) -> Pt2D {
        let (sin, cos) = theta.normalized_radians().sin_cos();
        Pt2D::new(
            self.x() + dist.inner_meters() * cos,
            self.y() + dist.inner_meters() * sin,
        )
    }

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

    pub fn center(pts: &[Pt2D]) -> Pt2D {
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

    pub fn to_hashable(self) -> HashablePt2D {
        HashablePt2D {
            x_nan: NotNan::new(self.x()).unwrap(),
            y_nan: NotNan::new(self.y()).unwrap(),
        }
    }

    /// Simplifies a list of points using Ramer-Douglas-Peuckr
    pub fn simplify_rdp(pts: Vec<Pt2D>, epsilon: f64) -> Vec<Pt2D> {
        let mut pts = pts_to_line_string(&pts)
            .simplify(&epsilon)
            .into_points()
            .into_iter()
            .map(|pt| pt.into())
            .collect::<Vec<_>>();
        // TODO Not sure why, but from geo 0.23 to 0.24, this became necessary?
        pts.dedup();
        pts
    }

    pub fn to_geojson(self, gps: Option<&GPSBounds>) -> geojson::Geometry {
        if let Some(gps) = gps {
            self.to_gps(gps).to_geojson()
        } else {
            geojson::Geometry::new(geojson::Value::Point(vec![self.x(), self.y()]))
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

impl From<Pt2D> for geo::Coord {
    fn from(pt: Pt2D) -> Self {
        geo::Coord { x: pt.x, y: pt.y }
    }
}

impl From<Pt2D> for geo::Point {
    fn from(pt: Pt2D) -> Self {
        geo::Point::new(pt.x, pt.y)
    }
}

impl From<geo::Coord> for Pt2D {
    fn from(coord: geo::Coord) -> Self {
        Pt2D::new(coord.x, coord.y)
    }
}

impl From<geo::Point> for Pt2D {
    fn from(point: geo::Point) -> Self {
        Pt2D::new(point.x(), point.y())
    }
}

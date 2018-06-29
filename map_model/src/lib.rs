// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate abstutil;
extern crate dimensioned;
extern crate geo;
extern crate graphics;
extern crate ordered_float;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;

mod building;
pub mod geometry;
mod intersection;
mod make;
mod map;
mod parcel;
mod polyline;
pub mod raw_data;
mod road;
mod turn;

pub use building::{Building, BuildingID};
use dimensioned::si;
use graphics::math::Vec2d;
pub use intersection::{Intersection, IntersectionID};
pub use map::Map;
use ordered_float::NotNaN;
pub use parcel::{Parcel, ParcelID};
pub use polyline::PolyLine;
use raw_data::LonLat;
pub use road::{LaneType, Road, RoadID};
use std::f64;
use std::fmt;
pub use turn::{Turn, TurnID};

// This isn't opinionated about what the (x, y) represents -- could be lat/lon or world space.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

// This represents world-space in meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt2D {
    x: f64,
    y: f64,
}

impl Pt2D {
    pub fn new(x: f64, y: f64) -> Pt2D {
        // TODO enforce after fixing:
        // - shift_polyline goes OOB sometimes
        // - convert_map uses this for GPS I think?
        if x < 0.0 || y < 0.0 {
            println!("Bad pt: {}, {}", x, y);
        }
        //assert!(x >= 0.0);
        //assert!(y >= 0.0);

        Pt2D { x, y }
    }

    pub fn from_gps(gps: &LonLat, b: &Bounds) -> Pt2D {
        // If not, havoc ensues
        assert!(b.contains(gps.longitude, gps.latitude));

        // Invert y, so that the northernmost latitude is 0. Screen drawing order, not Cartesian grid.
        let base = raw_data::LonLat::new(b.min_x, b.max_y);

        // Apparently the aabb_quadtree can't handle 0, so add a bit.
        // TODO epsilon or epsilon - 1.0?
        let dx = base.gps_dist_meters(LonLat::new(gps.longitude, base.latitude)) + f64::EPSILON;
        let dy = base.gps_dist_meters(LonLat::new(base.longitude, gps.latitude)) + f64::EPSILON;
        // By default, 1 meter is one pixel. Normal zooming can change that. If we did scaling here,
        // then we'd have to update all of the other constants too.
        Pt2D::new(dx, dy)
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }

    // TODO probably remove this
    pub fn to_vec(&self) -> Vec2d {
        [self.x(), self.y()]
    }

    // TODO better name
    // TODO Meters for dist?
    pub fn project_away(&self, dist: f64, theta: Angle) -> Pt2D {
        // If negative, caller should use theta.opposite()
        assert!(dist >= 0.0);

        let (sin, cos) = theta.0.sin_cos();
        Pt2D::new(self.x() + dist * cos, self.y() + dist * sin)
    }

    pub fn angle_to(&self, to: Pt2D) -> Angle {
        // DON'T invert y here
        Angle((to.y() - self.y()).atan2(to.x() - self.x()))
    }
}

impl fmt::Display for Pt2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pt2D({0}, {1})", self.x(), self.y())
    }
}

// TODO argh, use this in kml too
// TODO this maybe represents LonLat only?
#[derive(Clone, Copy, Debug)]
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

    pub fn update(&mut self, x: f64, y: f64) {
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
    }

    pub fn update_pt(&mut self, pt: &Pt2D) {
        self.update(pt.x(), pt.y());
    }

    pub fn update_coord(&mut self, coord: &raw_data::LonLat) {
        self.update(coord.longitude, coord.latitude);
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

// Stores in radians
#[derive(Clone, Copy, Debug)]
pub struct Angle(f64);

impl Angle {
    pub fn opposite(&self) -> Angle {
        Angle(self.0 + f64::consts::PI)
    }

    pub fn rotate_degs(&self, degrees: f64) -> Angle {
        Angle(self.0 + degrees.to_radians())
    }

    pub fn normalized_radians(&self) -> f64 {
        if self.0 < 0.0 {
            self.0 + (2.0 * f64::consts::PI)
        } else {
            self.0
        }
    }

    pub fn normalized_degrees(&self) -> f64 {
        self.normalized_radians().to_degrees()
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Angle({} degrees)", self.normalized_degrees())
    }
}

// Segment, technically
#[derive(Debug)]
pub struct Line(Pt2D, Pt2D);

impl Line {
    // TODO only one place outside this crate calls this, try to fix maybe?
    pub fn new(pt1: Pt2D, pt2: Pt2D) -> Line {
        Line(pt1, pt2)
    }

    // TODO we call these frequently here; unnecessary copies?
    pub fn pt1(&self) -> Pt2D {
        self.0
    }

    pub fn pt2(&self) -> Pt2D {
        self.1
    }

    // TODO valid to do euclidean distance on world-space points that're formed from
    // Haversine?
    pub fn length(&self) -> si::Meter<f64> {
        ((self.pt1().x() - self.pt2().x()).powi(2) + (self.pt1().y() - self.pt2().y()).powi(2))
            .sqrt() * si::M
    }

    pub fn intersection(&self, other: &Line) -> Option<Pt2D> {
        // TODO shoddy way of implementing this
        // TODO doesn't handle nearly parallel lines
        if !self.intersects(other) {
            None
        } else {
            polyline::line_intersection(self, other)
        }
    }

    pub fn shift(&self, width: f64) -> Line {
        let angle = self.pt1().angle_to(self.pt2()).rotate_degs(90.0);
        Line(
            self.pt1().project_away(width, angle),
            self.pt2().project_away(width, angle),
        )
    }

    pub fn reverse(&self) -> Line {
        Line(self.pt2(), self.pt1())
    }

    pub fn intersects(&self, other: &Line) -> bool {
        // From http://bryceboe.com/2006/10/23/line-segment-intersection-algorithm/
        is_counter_clockwise(self.pt1(), other.pt1(), other.pt2())
            != is_counter_clockwise(self.pt2(), other.pt1(), other.pt2())
            && is_counter_clockwise(self.pt1(), self.pt2(), other.pt1())
                != is_counter_clockwise(self.pt1(), self.pt2(), other.pt2())
    }

    pub fn angle(&self) -> Angle {
        self.pt1().angle_to(self.pt2())
    }

    pub fn dist_along(&self, dist: si::Meter<f64>) -> Pt2D {
        let len = self.length();
        if dist > len + geometry::EPSILON_METERS {
            panic!("cant do {} along a line of length {}", dist, len);
        }

        let percent = (dist / len).value_unsafe;
        Pt2D::new(
            self.pt1().x() + percent * (self.pt2().x() - self.pt1().x()),
            self.pt1().y() + percent * (self.pt2().y() - self.pt1().y()),
        )
        // TODO unit test
        /*
        let res_len = euclid_dist((pt1, &Pt2D::new(res[0], res[1])));
        if res_len != dist_along {
            println!("whats the delta btwn {} and {}?", res_len, dist_along);
        }
        */    }

    pub fn unbounded_dist_along(&self, dist: si::Meter<f64>) -> Pt2D {
        let len = self.length();
        let percent = (dist / len).value_unsafe;
        Pt2D::new(
            self.pt1().x() + percent * (self.pt2().x() - self.pt1().x()),
            self.pt1().y() + percent * (self.pt2().y() - self.pt1().y()),
        )
        // TODO unit test
        /*
        let res_len = euclid_dist((pt1, &Pt2D::new(res[0], res[1])));
        if res_len != dist_along {
            println!("whats the delta btwn {} and {}?", res_len, dist_along);
        }
        */    }
}

fn is_counter_clockwise(pt1: Pt2D, pt2: Pt2D, pt3: Pt2D) -> bool {
    (pt3.y() - pt1.y()) * (pt2.x() - pt1.x()) > (pt2.y() - pt1.y()) * (pt3.x() - pt1.x())
}

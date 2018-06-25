// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
#[macro_use]
extern crate dimensioned;
extern crate geo;
extern crate graphics;
extern crate ordered_float;
extern crate protobuf;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate vecmath;

mod building;
pub mod geometry;
mod intersection;
mod map;
mod parcel;
pub mod pb;
mod polyline;
mod road;
mod turn;

pub use building::{Building, BuildingID};
pub use geometry::angles::{Radian, RAD};
use graphics::math::Vec2d;
pub use intersection::{Intersection, IntersectionID};
pub use map::Map;
use ordered_float::NotNaN;
pub use parcel::{Parcel, ParcelID};
pub use polyline::{polygons_for_polyline, shift_polyline};
use protobuf::error::ProtobufError;
use protobuf::{CodedInputStream, CodedOutputStream, Message};
pub use road::{LaneType, Road, RoadID};
use std::f64;
use std::fmt;
use std::fs::File;
pub use turn::{Turn, TurnID};

pub fn write_pb(map: &pb::Map, path: &str) -> Result<(), ProtobufError> {
    let mut file = File::create(path)?;
    let mut cos = CodedOutputStream::new(&mut file);
    map.write_to(&mut cos)?;
    cos.flush()?;
    Ok(())
}

pub fn load_pb(path: &str) -> Result<pb::Map, ProtobufError> {
    let mut file = File::open(path)?;
    let mut cis = CodedInputStream::new(&mut file);
    let mut map = pb::Map::new();
    map.merge_from(&mut cis)?;
    Ok(map)
}

// This isn't opinionated about what the (x, y) represents. Could be GPS coordinates, could be
// screen-space.
// TODO but actually, different types to represent GPS and screen space would be awesome.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Pt2D {
    x_nan: NotNaN<f64>,
    y_nan: NotNaN<f64>,
}

impl Pt2D {
    pub fn new(x: f64, y: f64) -> Pt2D {
        Pt2D {
            x_nan: NotNaN::new(x).unwrap(),
            y_nan: NotNaN::new(y).unwrap(),
        }
    }

    pub fn zero() -> Pt2D {
        Pt2D::new(0.0, 0.0)
    }

    pub fn x(&self) -> f64 {
        self.x_nan.into_inner()
    }

    pub fn y(&self) -> f64 {
        self.y_nan.into_inner()
    }

    // Interprets the Pt2D as GPS coordinates, using Haversine distance
    pub fn gps_dist_meters(&self, other: &Pt2D) -> f64 {
        let earth_radius_m = 6371000.0;
        let lon1 = self.x().to_radians();
        let lon2 = other.x().to_radians();
        let lat1 = self.y().to_radians();
        let lat2 = other.y().to_radians();

        let delta_lat = lat2 - lat1;
        let delta_lon = lon2 - lon1;

        let a = (delta_lat / 2.0).sin().powi(2)
            + (delta_lon / 2.0).sin().powi(2) * lat1.cos() * lat2.cos();
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        earth_radius_m * c
    }

    pub fn to_vec(&self) -> Vec2d {
        [self.x(), self.y()]
    }
}

impl<'a> From<&'a pb::Coordinate> for Pt2D {
    fn from(pt: &pb::Coordinate) -> Self {
        Pt2D::new(pt.get_longitude(), pt.get_latitude())
    }
}

impl From<[f64; 2]> for Pt2D {
    fn from(pt: [f64; 2]) -> Self {
        Pt2D::new(pt[0], pt[1])
    }
}

impl fmt::Display for Pt2D {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Pt2D({0}, {1})", self.x(), self.y())
    }
}

// TODO argh, use this in kml too
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

    pub fn update_coord(&mut self, coord: &pb::Coordinate) {
        self.update(coord.get_longitude(), coord.get_latitude());
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }
}

pub fn has_osm_tag(tags: &Vec<String>, key: &str, value: &str) -> bool {
    tags.contains(&format!("{}={}", key, value))
}

fn get_gps_bounds(data: &pb::Map) -> Bounds {
    let mut bounds = Bounds::new();

    for r in data.get_roads() {
        for pt in r.get_points() {
            bounds.update_coord(pt);
        }
    }
    for i in data.get_intersections() {
        bounds.update_coord(i.get_point());
    }
    for b in data.get_buildings() {
        for pt in b.get_points() {
            bounds.update_coord(pt);
        }
    }
    for p in data.get_parcels() {
        for pt in p.get_points() {
            bounds.update_coord(pt);
        }
    }

    bounds
}

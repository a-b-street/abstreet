extern crate aabb_quadtree;
extern crate abstutil;
#[macro_use]
extern crate derivative;
extern crate dimensioned;
extern crate geo;
extern crate geom;
extern crate gtfs;
#[macro_use]
extern crate log;
extern crate multimap;
extern crate ordered_float;
#[macro_use]
extern crate pretty_assertions;
extern crate serde;
#[macro_use]
extern crate serde_derive;

#[macro_use]
mod macros;

mod area;
mod building;
mod bus_stop;
mod edits;
mod intersection;
mod lane;
mod make;
mod map;
mod parcel;
mod pathfind;
pub mod raw_data;
mod road;
mod traversable;
mod turn;

pub use area::{Area, AreaID, AreaType};
pub use building::{Building, BuildingID, FrontPath};
pub use bus_stop::{BusRoute, BusStop, BusStopID};
pub use edits::{EditReason, RoadEdits};
pub use intersection::{Intersection, IntersectionID};
pub use lane::{Lane, LaneID, LaneType, PARKING_SPOT_LENGTH};
pub use map::Map;
pub use parcel::{Parcel, ParcelID};
pub use pathfind::{Path, PathStep, Pathfinder, Trace};
pub use road::{Road, RoadID};
pub use traversable::Traversable;
pub use turn::{Turn, TurnID};

pub const LANE_THICKNESS: f64 = 2.5;

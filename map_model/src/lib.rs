extern crate abstutil;
extern crate dimensioned;
#[macro_use]
extern crate failure;
extern crate flame;
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
pub use pathfind::Pathfinder;
pub use road::{Road, RoadID};
pub use traversable::Traversable;
pub use turn::{Turn, TurnID};

pub const LANE_THICKNESS: f64 = 2.5;

#[derive(Debug, Fail)]
#[fail(display = "{}", reason)]
pub struct MapError {
    reason: String,
}

impl MapError {
    pub fn new(reason: String) -> MapError {
        MapError { reason }
    }
}

extern crate aabb_quadtree;
extern crate abstutil;
extern crate dimensioned;
extern crate flame;
extern crate geo;
extern crate geom;
extern crate gtfs;
extern crate multimap;
extern crate ordered_float;
#[macro_use]
extern crate pretty_assertions;
extern crate serde;
#[macro_use]
extern crate serde_derive;

mod building;
mod edits;
pub mod geometry;
mod intersection;
mod lane;
mod make;
mod map;
mod parcel;
mod pathfind;
pub mod raw_data;
mod road;
mod turn;

pub use building::{Building, BuildingID, FrontPath};
pub use edits::{EditReason, Edits};
pub use intersection::{Intersection, IntersectionID};
pub use lane::{BusStop, BusStopDetails, Lane, LaneID, LaneType, PARKING_SPOT_LENGTH};
pub use map::Map;
pub use parcel::{Parcel, ParcelID};
pub use pathfind::pathfind;
pub use road::{Road, RoadID};
pub use turn::{Turn, TurnID};

// TODO This sort of doesn't fit in the map layer, but it's quite convenient to store it.
#[derive(Serialize, Deserialize, Debug)]
pub struct BusRoute {
    pub name: String,
    pub stops: Vec<BusStop>,
}

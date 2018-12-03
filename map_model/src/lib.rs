extern crate aabb_quadtree;
extern crate abstutil;
extern crate dimensioned;
extern crate geo;
extern crate geom;
extern crate gtfs;
#[macro_use]
extern crate log;
extern crate multimap;
extern crate nbez;
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
mod find_closest;
mod intersection;
mod lane;
mod make;
mod map;
mod parcel;
mod pathfind;
pub mod raw_data;
mod road;
mod stop_signs;
mod traffic_signals;
mod traversable;
mod turn;

use abstutil::Cloneable;
pub use area::{Area, AreaID, AreaType};
pub use building::{Building, BuildingID, FrontPath};
pub use bus_stop::{BusRoute, BusRouteID, BusStop, BusStopID};
pub use edits::{EditReason, MapEdits};
pub use find_closest::FindClosest;
pub use intersection::{Intersection, IntersectionID, IntersectionType};
pub use lane::{Lane, LaneID, LaneType, PARKING_SPOT_LENGTH};
pub use make::RoadSpec;
pub use map::Map;
pub use parcel::{Parcel, ParcelID};
pub use pathfind::{Path, PathRequest, PathStep, Pathfinder, Trace};
pub use road::{Road, RoadID};
pub use stop_signs::ControlStopSign;
pub use traffic_signals::{ControlTrafficSignal, Cycle};
pub use traversable::{Position, Traversable};
pub use turn::{Turn, TurnID, TurnPriority, TurnType};

pub const LANE_THICKNESS: f64 = 2.5;

impl Cloneable for IntersectionID {}
impl Cloneable for MapEdits {}

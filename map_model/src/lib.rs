//! `map_model` describes the world where simulations occur. Importing a map from OSM partly happens
//! in `convert_osm` and here.
//!
//! Helpful terminology:
//! - ch = contraction hierarchy, for speeding up pathfinding
//! - degenerate intersection = only has 2 roads connected, so why is it an intersection at all?
//! - lc = lane-change (which is modelled very strangely: <https://a-b-street.github.io/docs/trafficsim/discrete_event.html#lane-changing>)
//! - ltr = left-to-right, the order of lanes for a road
//! - osm = OpenStreetMap
//!
//! Map objects are usually abbreviated in method names:
//! - a = area
//! - b = building
//! - br = bus route
//! - bs = bus stop
//! - i = intersection
//! - l = lane
//! - pl = parking lot
//! - r = road
//! - ss = stop sign
//! - t = turn
//! - ts = traffic signal

#![allow(clippy::new_without_default)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Bounds, GPSBounds, Polygon};

pub use crate::city::City;
pub use crate::edits::{
    EditCmd, EditEffects, EditIntersection, EditRoad, MapEdits, PermanentMapEdits,
};
pub use crate::make::RawToMapOptions;
pub use crate::map::{DrivingSide, MapConfig};
pub use crate::objects::area::{Area, AreaID, AreaType};
pub use crate::objects::building::{
    Amenity, AmenityType, Building, BuildingID, BuildingType, NamePerLanguage, OffstreetParking,
};
pub use crate::objects::bus_stop::{BusRoute, BusRouteID, BusStop, BusStopID};
pub use crate::objects::intersection::{Intersection, IntersectionID, IntersectionType};
pub use crate::objects::lane::{
    Lane, LaneID, LaneSpec, LaneType, NORMAL_LANE_THICKNESS, PARKING_LOT_SPOT_LENGTH,
    SIDEWALK_THICKNESS,
};
pub use crate::objects::parking_lot::{ParkingLot, ParkingLotID};
pub use crate::objects::road::{DirectedRoadID, Direction, Road, RoadID};
pub use crate::objects::stop_signs::{ControlStopSign, RoadWithStopSign};
pub use crate::objects::traffic_signals::{ControlTrafficSignal, Stage, StageType};
pub use crate::objects::turn::{
    CompressedMovementID, Movement, MovementID, Turn, TurnID, TurnPriority, TurnType,
};
pub use crate::objects::zone::{AccessRestrictions, Zone};
pub use crate::pathfind::uber_turns::{IntersectionCluster, UberTurn};
use crate::pathfind::Pathfinder;
pub use crate::pathfind::{
    Path, PathConstraints, PathRequest, PathStep, PathStepV2, PathV2, RoutingParams,
};
pub use crate::traversable::{Position, Traversable, MAX_BIKE_SPEED, MAX_WALKING_SPEED};

mod city;
pub mod connectivity;
mod edits;
mod make;
mod map;
mod objects;
pub mod osm;
mod pathfind;
pub mod raw;
mod traversable;

// The map used by the simulation and UI. This struct is declared here so that the rest of the
// crate can reach into private fields.
#[derive(Serialize, Deserialize)]
pub struct Map {
    roads: Vec<Road>,
    lanes: BTreeMap<LaneID, Lane>,
    lane_id_counter: usize,
    intersections: Vec<Intersection>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    turns: BTreeMap<TurnID, Turn>,
    buildings: Vec<Building>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    bus_stops: BTreeMap<BusStopID, BusStop>,
    bus_routes: Vec<BusRoute>,
    areas: Vec<Area>,
    parking_lots: Vec<ParkingLot>,
    boundary_polygon: Polygon,

    // Note that border nodes belong in neither!
    stop_signs: BTreeMap<IntersectionID, ControlStopSign>,
    traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal>,

    gps_bounds: GPSBounds,
    bounds: Bounds,
    config: MapConfig,

    pathfinder: Pathfinder,
    pathfinder_dirty: bool,
    routing_params: RoutingParams,
    // Not the source of truth, just cached.
    zones: Vec<Zone>,

    name: MapName,
    #[serde(skip_serializing, skip_deserializing)]
    edits: MapEdits,
}

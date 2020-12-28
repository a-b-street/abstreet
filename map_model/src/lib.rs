//! `map_model` describes the world where simulations occur. Importing a map from OSM partly happens
//! in `convert_osm` and here.
//!
//! Helpful terminology:
//! - ch = contraction hierarchy, for speeding up pathfinding
//! - degenerate intersection = only has 2 roads connected, so why is it an intersection at all?
//! - lc = lane-change (which is modelled very strangely: <https://dabreegster.github.io/abstreet/trafficsim/discrete_event.html#lane-changing>)
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

#[macro_use]
extern crate log;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap, MapName};
use geom::{Bounds, Distance, GPSBounds, Polygon};

pub use crate::city::City;
pub use crate::edits::{
    EditCmd, EditEffects, EditIntersection, EditRoad, MapEdits, PermanentMapEdits,
};
pub use crate::map::{DrivingSide, MapConfig};
pub use crate::objects::area::{Area, AreaID, AreaType};
pub use crate::objects::building::{
    Amenity, AmenityType, Building, BuildingID, BuildingType, NamePerLanguage, OffstreetParking,
};
pub use crate::objects::bus_stop::{BusRoute, BusRouteID, BusStop, BusStopID};
pub use crate::objects::intersection::{Intersection, IntersectionID, IntersectionType};
pub use crate::objects::lane::{
    Lane, LaneID, LaneType, PARKING_LOT_SPOT_LENGTH, PARKING_SPOT_LENGTH,
};
pub use crate::objects::parking_lot::{ParkingLot, ParkingLotID};
pub use crate::objects::road::{DirectedRoadID, Direction, Road, RoadID};
pub use crate::objects::stop_signs::{ControlStopSign, RoadWithStopSign};
pub use crate::objects::traffic_signals::{ControlTrafficSignal, PhaseType, Stage};
pub use crate::objects::turn::{
    CompressedMovementID, Movement, MovementID, Turn, TurnID, TurnPriority, TurnType,
};
pub use crate::objects::zone::{AccessRestrictions, Zone};
pub use crate::pathfind::uber_turns::{IntersectionCluster, UberTurn, UberTurnGroup};
use crate::pathfind::Pathfinder;
pub use crate::pathfind::{Path, PathConstraints, PathRequest, PathStep};
pub use crate::traversable::{Position, Traversable};

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

// TODO Minimize uses of these!
pub const NORMAL_LANE_THICKNESS: Distance = Distance::const_meters(2.5);
pub(crate) const SERVICE_ROAD_LANE_THICKNESS: Distance = Distance::const_meters(1.5);
pub const SIDEWALK_THICKNESS: Distance = Distance::const_meters(1.5);
pub(crate) const SHOULDER_THICKNESS: Distance = Distance::const_meters(0.5);

// The map used by the simulation and UI. This struct is declared here so that the rest of the
// crate can reach into private fields.
#[derive(Serialize, Deserialize)]
pub struct Map {
    roads: Vec<Road>,
    lanes: Vec<Lane>,
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
    // Not the source of truth, just cached.
    zones: Vec<Zone>,

    name: MapName,
    #[serde(skip_serializing, skip_deserializing)]
    edits: MapEdits,
}

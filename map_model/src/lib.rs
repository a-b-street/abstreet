//! `map_model` describes the world where simulations occur. Importing a map from OSM partly happens
//! in `convert_osm` and here.
//!
//! Helpful terminology:
//! - ch = contraction hierarchy, for speeding up pathfinding
//! - degenerate intersection = only has 2 roads connected, so why is it an intersection at all?
//! - lc = lane-change (which is modelled very strangely: <https://a-b-street.github.io/docs/tech/trafficsim/discrete_event/index.html#lane-changing>)
//! - ltr = left-to-right, the order of lanes for a road
//! - osm = OpenStreetMap
//!
//! Map objects are usually abbreviated in method names:
//! - a = area
//! - b = building
//! - tr = transit route
//! - ts = transit stop
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

use popgetter::CensusZone;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::{
    deserialize_btreemap, deserialize_multimap, serialize_btreemap, serialize_multimap, MultiMap,
};
use geom::{Bounds, GPSBounds, Polygon};
pub use osm2streets::{
    osm, BufferType, Direction, DrivingSide, IntersectionControl, IntersectionKind, LaneSpec,
    LaneType, MapConfig, NamePerLanguage, RestrictionType, NORMAL_LANE_THICKNESS,
    SIDEWALK_THICKNESS,
};
pub use raw_map::{Amenity, AmenityType, AreaType, CrossingType, ExtraPOI, ExtraPOIType};

pub use crate::city::City;
pub use crate::edits::{
    EditCmd, EditEffects, EditIntersection, EditRoad, MapEdits, PermanentMapEdits,
};
pub use crate::make::RawToMapOptions;
pub use crate::objects::area::{Area, AreaID};
pub use crate::objects::building::{Building, BuildingID, BuildingType, OffstreetParking};
pub use crate::objects::intersection::{Intersection, IntersectionID};
pub use crate::objects::lane::{CommonEndpoint, Lane, LaneID, PARKING_LOT_SPOT_LENGTH};
pub use crate::objects::movement::{CompressedMovementID, Movement, MovementID};
pub use crate::objects::parking_lot::{ParkingLot, ParkingLotID};
pub use crate::objects::road::{
    DirectedRoadID, OriginalRoad, Road, RoadID, RoadSideID, SideOfRoad,
};
pub use crate::objects::stop_signs::{ControlStopSign, RoadWithStopSign};
pub use crate::objects::traffic_signals::{ControlTrafficSignal, Stage, StageType};
pub use crate::objects::transit::{TransitRoute, TransitRouteID, TransitStop, TransitStopID};
pub use crate::objects::turn::{Turn, TurnID, TurnPriority, TurnType};
pub use crate::objects::zone::{AccessRestrictions, Zone};
pub use crate::pathfind::uber_turns::{IntersectionCluster, UberTurn};
pub use crate::pathfind::{
    Path, PathConstraints, PathRequest, PathStep, PathStepV2, PathV2, Pathfinder, PathfinderCache,
    PathfinderCaching, RoutingParams,
};
pub use crate::traversable::{Position, Traversable, MAX_BIKE_SPEED, MAX_WALKING_SPEED};

mod city;
pub mod connectivity;
mod edits;
mod make;
mod map;
mod objects;
mod pathfind;
mod traversable;

// The map used by the simulation and UI. This struct is declared here so that the rest of the
// crate can reach into private fields.
#[derive(Clone, Serialize, Deserialize)]
pub struct Map {
    roads: Vec<Road>,
    intersections: Vec<Intersection>,
    buildings: Vec<Building>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    transit_stops: BTreeMap<TransitStopID, TransitStop>,
    transit_routes: Vec<TransitRoute>,
    areas: Vec<Area>,
    parking_lots: Vec<ParkingLot>,
    boundary_polygon: Polygon,

    // Note that border nodes belong in neither!
    stop_signs: BTreeMap<IntersectionID, ControlStopSign>,
    traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal>,

    #[serde(
        serialize_with = "serialize_multimap",
        deserialize_with = "deserialize_multimap"
    )]
    bus_routes_on_roads: MultiMap<osm::WayID, String>,

    gps_bounds: GPSBounds,
    bounds: Bounds,
    config: MapConfig,

    pathfinder: Pathfinder,
    pathfinder_dirty: bool,
    routing_params: RoutingParams,
    // Not the source of truth, just cached.
    zones: Vec<Zone>,
    census_zones: Vec<(Polygon, CensusZone)>,
    extra_pois: Vec<ExtraPOI>,

    name: MapName,

    #[serde(skip_serializing, skip_deserializing)]
    edits: MapEdits,
    #[serde(skip_serializing, skip_deserializing)]
    edits_generation: usize,
    #[serde(skip_serializing, skip_deserializing)]
    road_to_buildings: MultiMap<RoadID, BuildingID>,
}

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

pub use crate::city::City;
pub use crate::edits::{
    EditCmd, EditEffects, EditIntersection, MapEdits, OriginalLane, PermanentMapEdits,
};
pub use crate::make::initial::lane_specs::RoadSpec;
pub use crate::map::MapConfig;
pub use crate::objects::area::{Area, AreaID, AreaType};
pub use crate::objects::building::{Building, BuildingID, FrontPath, OffstreetParking};
pub use crate::objects::bus_stop::{BusRoute, BusRouteID, BusStop, BusStopID};
pub use crate::objects::intersection::{Intersection, IntersectionID, IntersectionType};
pub use crate::objects::lane::{
    Lane, LaneID, LaneType, PARKING_LOT_SPOT_LENGTH, PARKING_SPOT_LENGTH,
};
pub use crate::objects::parking_lot::{ParkingLot, ParkingLotID};
pub use crate::objects::road::{DirectedRoadID, Road, RoadID};
pub use crate::objects::stop_signs::{ControlStopSign, RoadWithStopSign};
pub use crate::objects::traffic_signals::{ControlTrafficSignal, Phase, PhaseType};
pub use crate::objects::turn::{Turn, TurnGroup, TurnGroupID, TurnID, TurnPriority, TurnType};
pub use crate::objects::zone::Zone;
pub use crate::pathfind::uber_turns::{IntersectionCluster, UberTurn, UberTurnGroup};
use crate::pathfind::Pathfinder;
pub use crate::pathfind::{Path, PathConstraints, PathRequest, PathStep};
pub use crate::traversable::{Position, Traversable};
use abstutil::Cloneable;
use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Bounds, Distance, GPSBounds, Polygon};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// TODO Minimize uses of these!
pub const NORMAL_LANE_THICKNESS: Distance = Distance::const_meters(2.5);
pub const SIDEWALK_THICKNESS: Distance = Distance::const_meters(1.5);

impl Cloneable for BusRouteID {}
impl Cloneable for ControlTrafficSignal {}
impl Cloneable for IntersectionID {}
impl Cloneable for LaneType {}
impl Cloneable for MapEdits {}
impl Cloneable for raw::RestrictionType {}

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

    // TODO Argh, hack, initialization order is hard!
    pathfinder: Option<Pathfinder>,
    pathfinder_dirty: bool,
    // Not the source of truth, just cached.
    zones: Vec<Zone>,

    city_name: String,
    name: String,
    #[serde(skip_serializing, skip_deserializing)]
    edits: MapEdits,
}

mod area;
mod building;
mod bus_stop;
pub mod connectivity;
mod edits;
mod intersection;
mod lane;
mod make;
mod map;
mod neighborhood;
pub mod osm;
mod pathfind;
pub mod raw;
mod road;
mod stop_signs;
mod traffic_signals;
mod traversable;
mod turn;

pub use crate::area::{Area, AreaID, AreaType};
pub use crate::building::{Building, BuildingID, FrontPath, OffstreetParking};
pub use crate::bus_stop::{BusRoute, BusRouteID, BusStop, BusStopID};
pub use crate::edits::{EditCmd, EditEffects, MapEdits};
pub use crate::intersection::{Intersection, IntersectionID, IntersectionType};
pub use crate::lane::{Lane, LaneID, LaneType, PARKING_SPOT_LENGTH};
pub use crate::make::RoadSpec;
pub use crate::map::Map;
pub use crate::neighborhood::{FullNeighborhoodInfo, Neighborhood, NeighborhoodBuilder};
pub use crate::pathfind::{Path, PathConstraints, PathRequest, PathStep};
pub use crate::road::{DirectedRoadID, Road, RoadID};
pub use crate::stop_signs::{ControlStopSign, RoadWithStopSign};
pub use crate::traffic_signals::{ControlTrafficSignal, Phase};
pub use crate::traversable::{Position, Traversable};
pub use crate::turn::{Turn, TurnGroup, TurnGroupID, TurnID, TurnPriority, TurnType};
use abstutil::Cloneable;
use geom::Distance;

// TODO Minimize uses of these!
pub const NORMAL_LANE_THICKNESS: Distance = Distance::const_meters(2.5);
pub const SIDEWALK_THICKNESS: Distance = Distance::const_meters(1.5);

impl Cloneable for BusRouteID {}
impl Cloneable for ControlTrafficSignal {}
impl Cloneable for IntersectionID {}
impl Cloneable for LaneType {}
impl Cloneable for MapEdits {}
impl Cloneable for Neighborhood {}
impl Cloneable for NeighborhoodBuilder {}
impl Cloneable for raw::RestrictionType {}

mod area;
mod building;
mod bus_stop;
mod edits;
mod intersection;
mod lane;
mod make;
mod map;
mod neighborhood;
mod pathfind;
pub mod raw_data;
mod road;
mod stop_signs;
mod traffic_signals;
mod traversable;
mod turn;

pub use crate::area::{Area, AreaID, AreaType};
pub use crate::building::{Building, BuildingID, BuildingType, FrontPath};
pub use crate::bus_stop::{BusRoute, BusRouteID, BusStop, BusStopID};
pub use crate::edits::MapEdits;
pub use crate::intersection::{Intersection, IntersectionID, IntersectionType};
pub use crate::lane::{Lane, LaneID, LaneType, PARKING_SPOT_LENGTH};
pub use crate::make::RoadSpec;
pub use crate::map::Map;
pub use crate::neighborhood::{FullNeighborhoodInfo, Neighborhood, NeighborhoodBuilder};
pub use crate::pathfind::{Path, PathRequest, PathStep};
pub use crate::road::{DirectedRoadID, Road, RoadID};
pub use crate::stop_signs::ControlStopSign;
pub use crate::traffic_signals::{ControlTrafficSignal, Cycle};
pub use crate::traversable::{Position, Traversable};
pub use crate::turn::{Turn, TurnID, TurnPriority, TurnType};
use abstutil::Cloneable;
use geom::Distance;

pub const LANE_THICKNESS: Distance = Distance::const_meters(2.5);

impl Cloneable for ControlTrafficSignal {}
impl Cloneable for IntersectionID {}
impl Cloneable for MapEdits {}
impl Cloneable for Neighborhood {}
impl Cloneable for NeighborhoodBuilder {}

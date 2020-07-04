use crate::{LaneID, Position};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BusStopID {
    pub sidewalk: LaneID,
    // As long as this is unique per lane, this value is otherwise meaningless. Not contiguous or
    // ordered in any way.
    pub(crate) idx: usize,
}

impl fmt::Display for BusStopID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BusStopID({0}, {1})", self.sidewalk, self.idx)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BusRouteID(pub usize);

impl fmt::Display for BusRouteID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BusRoute #{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BusStop {
    pub id: BusStopID,
    pub name: String,
    // These may be on different roads entirely, like for light rail platforms.
    pub driving_pos: Position,
    pub sidewalk_pos: Position,
    // If it's both, train overrides bus
    pub is_train_stop: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BusRoute {
    pub id: BusRouteID,
    pub name: String,
    pub stops: Vec<BusStopID>,
}

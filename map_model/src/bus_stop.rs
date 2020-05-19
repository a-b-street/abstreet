use crate::{LaneID, Position};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BusStopID {
    pub sidewalk: LaneID,
    // This might actually not be contiguous and correct; we could remove a stop in between two
    // others
    pub idx: usize,
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
    // These might be on opposite sides of the road in the case of one-ways. Shouldn't matter
    // anywhere.
    pub driving_pos: Position,
    pub sidewalk_pos: Position,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BusRoute {
    pub id: BusRouteID,
    pub name: String,
    pub stops: Vec<BusStopID>,
}

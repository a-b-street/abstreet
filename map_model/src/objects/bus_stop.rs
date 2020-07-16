use crate::{LaneID, Map, PathConstraints, PathRequest, Position};
use abstutil::{deserialize_usize, serialize_usize};
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
pub struct BusRouteID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

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
    pub full_name: String,
    pub short_name: String,
    pub stops: Vec<BusStopID>,
    pub start_border: Option<LaneID>,
    pub end_border: Option<LaneID>,
    pub route_type: PathConstraints,
}

impl BusRoute {
    pub fn all_steps(&self, map: &Map) -> Vec<PathRequest> {
        let mut steps = Vec::new();
        if let Some(start) = self.start_border {
            steps.push(PathRequest {
                start: Position::start(start),
                end: map.get_bs(self.stops[0]).driving_pos,
                constraints: self.route_type,
            });
        }
        for pair in self.stops.windows(2) {
            steps.push(PathRequest {
                start: map.get_bs(pair[0]).driving_pos,
                end: map.get_bs(pair[1]).driving_pos,
                constraints: self.route_type,
            });
        }
        if let Some(end) = self.end_border {
            steps.push(PathRequest {
                start: map.get_bs(*self.stops.last().unwrap()).driving_pos,
                end: Position::end(end, map),
                constraints: self.route_type,
            });
        }
        steps
    }
}

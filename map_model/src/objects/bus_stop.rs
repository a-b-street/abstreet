use crate::{LaneID, Map, PathConstraints, PathRequest, Position};
use abstutil::{deserialize_usize, serialize_usize};
use geom::Time;
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
    pub gtfs_trip_marker: Option<String>,
    pub osm_rel_id: i64,
    pub stops: Vec<BusStopID>,
    // May be a border or not. If not, is long enough for buses to spawn fully.
    pub start: LaneID,
    pub end_border: Option<LaneID>,
    pub route_type: PathConstraints,
    // Non-empty, times in order for one day when a vehicle should begin at start.
    pub spawn_times: Vec<Time>,
    // Explicitly store whatever the original was, since this can't be reconstructed without side
    // input.
    pub orig_spawn_times: Vec<Time>,
}

impl BusRoute {
    pub fn all_steps(&self, map: &Map) -> Vec<PathRequest> {
        let mut steps = Vec::new();
        steps.push(PathRequest {
            start: Position::start(self.start),
            end: map.get_bs(self.stops[0]).driving_pos,
            constraints: self.route_type,
        });
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

    pub fn plural_noun(&self) -> &'static str {
        if self.route_type == PathConstraints::Bus {
            "buses"
        } else {
            "trains"
        }
    }
}

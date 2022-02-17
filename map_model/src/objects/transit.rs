//! Public transit stops and routes.

use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use geom::Time;

use crate::{LaneID, Map, PathConstraints, PathRequest, Position, RoadID};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TransitStopID {
    pub road: RoadID,
    /// As long as this is unique per road, this value is otherwise meaningless. Not contiguous or
    /// ordered in any way.
    pub(crate) idx: usize,
}

impl fmt::Display for TransitStopID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TransitStopID({0}, {1})", self.road, self.idx)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransitRouteID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for TransitRouteID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TransitRoute #{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TransitStop {
    pub id: TransitStopID,
    pub name: String,
    pub gtfs_id: String,
    /// These may be on different roads entirely, like for light rail platforms.
    pub driving_pos: Position,
    pub sidewalk_pos: Position,
    /// If false, only buses serve this stop
    pub is_train_stop: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TransitRoute {
    pub id: TransitRouteID,
    pub long_name: String,
    pub short_name: String,
    pub gtfs_id: String,
    pub stops: Vec<TransitStopID>,
    /// A transit vehicle spawns at the beginning of this lane. This lane may be at a border or the
    /// first stop. For the non-border case, the lane must be long enough for the vehicle to spawn.
    pub start: LaneID,
    /// A transit vehicle either vanishes at its last stop or exits the map through this border.
    pub end_border: Option<LaneID>,
    pub route_type: PathConstraints,
    /// Non-empty, times in order for one day when a vehicle should begin at start.
    pub spawn_times: Vec<Time>,
    /// Explicitly store whatever the original was, since this can't be reconstructed without side
    /// input.
    pub orig_spawn_times: Vec<Time>,
}

impl TransitRoute {
    pub fn all_path_requests(&self, map: &Map) -> Vec<PathRequest> {
        let mut steps = vec![PathRequest::vehicle(
            Position::start(self.start),
            map.get_ts(self.stops[0]).driving_pos,
            self.route_type,
        )];
        for pair in self.stops.windows(2) {
            steps.push(PathRequest::vehicle(
                map.get_ts(pair[0]).driving_pos,
                map.get_ts(pair[1]).driving_pos,
                self.route_type,
            ));
        }
        if let Some(end) = self.end_border {
            steps.push(PathRequest::vehicle(
                map.get_ts(*self.stops.last().unwrap()).driving_pos,
                Position::end(end, map),
                self.route_type,
            ));
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

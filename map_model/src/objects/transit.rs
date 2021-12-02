//! Public transit stops and routes.

use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use geom::Time;

use crate::{osm, LaneID, Map, PathConstraints, PathRequest, Position};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TransitStopID {
    pub sidewalk: LaneID,
    /// As long as this is unique per lane, this value is otherwise meaningless. Not contiguous or
    /// ordered in any way.
    pub(crate) idx: usize,
}

impl fmt::Display for TransitStopID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TransitStopID({0}, {1})", self.sidewalk, self.idx)
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
    pub gtfs_trip_marker: Option<String>,
    pub osm_rel_id: osm::RelationID,
    pub stops: Vec<TransitStopID>,
    /// May be a border or not. If not, is long enough for buses to spawn fully.
    pub start: LaneID,
    pub end_border: Option<LaneID>,
    pub route_type: PathConstraints,
    /// Non-empty, times in order for one day when a vehicle should begin at start.
    pub spawn_times: Vec<Time>,
    /// Explicitly store whatever the original was, since this can't be reconstructed without side
    /// input.
    pub orig_spawn_times: Vec<Time>,
}

impl TransitRoute {
    pub fn all_steps(&self, map: &Map) -> Vec<PathRequest> {
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

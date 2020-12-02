use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;

use crate::pathfind::ch::ContractionHierarchyPathfinder;
use crate::pathfind::dijkstra;
use crate::{BusRouteID, BusStopID, LaneID, Map, Path, PathRequest, Position};

/// Most of the time, prefer using the faster contraction hierarchies. But sometimes, callers can
/// explicitly opt into a slower (but preparation-free) pathfinder that just uses Dijkstra's
/// maneuever.
#[derive(Serialize, Deserialize)]
pub enum Pathfinder {
    Dijkstra,
    CH(ContractionHierarchyPathfinder),
}

impl Pathfinder {
    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        match self {
            Pathfinder::Dijkstra => dijkstra::simple_pathfind(&req, map),
            Pathfinder::CH(ref p) => p.pathfind(req, map),
        }
    }

    pub fn pathfind_avoiding_lanes(
        &self,
        req: PathRequest,
        avoid: BTreeSet<LaneID>,
        map: &Map,
    ) -> Option<Path> {
        dijkstra::pathfind_avoiding_lanes(req, avoid, map)
    }

    // TODO Consider returning the walking-only path in the failure case, to avoid wasting work
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        match self {
            // TODO Implement this
            Pathfinder::Dijkstra => None,
            Pathfinder::CH(ref p) => p.should_use_transit(map, start, end),
        }
    }

    pub fn apply_edits(&mut self, map: &Map, timer: &mut Timer) {
        match self {
            Pathfinder::Dijkstra => {}
            Pathfinder::CH(ref mut p) => p.apply_edits(map, timer),
        }
    }
}

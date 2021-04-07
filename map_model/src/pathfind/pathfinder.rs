use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;

use crate::pathfind::ch::ContractionHierarchyPathfinder;
use crate::pathfind::dijkstra;
use crate::{BusRouteID, BusStopID, Map, Path, PathRequest, Position, RoadID, RoutingParams};

/// Most of the time, prefer using the faster contraction hierarchies. But sometimes, callers can
/// explicitly opt into a slower (but preparation-free) pathfinder that just uses Dijkstra's
/// maneuever.
#[derive(Serialize, Deserialize)]
pub enum Pathfinder {
    Dijkstra,
    CH(ContractionHierarchyPathfinder),
}

impl Pathfinder {
    /// Finds a path from a start to an end for a certain type of agent.
    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        self.pathfind_with_params(req, map.routing_params(), map)
    }

    /// Finds a path from a start to an end for a certain type of agent. May use custom routing
    /// parameters.
    pub fn pathfind_with_params(
        &self,
        req: PathRequest,
        params: &RoutingParams,
        map: &Map,
    ) -> Option<Path> {
        if params != map.routing_params() {
            // If the params differ from the ones baked into the map, the CHs won't match. This
            // should only be happening from the debug UI; be very obnoxious if we start calling it
            // from the simulation or something else.
            warn!("Pathfinding slowly for {} with custom params", req);
            return dijkstra::pathfind(req, params, map).map(|(path, _)| path);
        }

        match self {
            Pathfinder::Dijkstra => dijkstra::pathfind(req, params, map).map(|(path, _)| path),
            Pathfinder::CH(ref p) => p.pathfind(req, map),
        }
    }

    /// Note this is a slower implementation, never using contraction hierarchies. Used for
    /// experimental congestion capping.
    pub fn pathfind_avoiding_roads(
        &self,
        req: PathRequest,
        avoid: BTreeSet<RoadID>,
        map: &Map,
    ) -> Option<Path> {
        dijkstra::pathfind_avoiding_roads(req, avoid, map).map(|(path, _)| path)
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

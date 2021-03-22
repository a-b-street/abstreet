use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;

use crate::pathfind::ch::ContractionHierarchyPathfinder;
use crate::pathfind::dijkstra;
use crate::pathfind::walking::{one_step_walking_path, walking_path_to_steps};
use crate::{
    BusRouteID, BusStopID, LaneID, Map, Path, PathConstraints, PathRequest, Position, RoutingParams,
};

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
        if req.start.lane() == req.end.lane() && req.constraints == PathConstraints::Pedestrian {
            return Some(one_step_walking_path(&req, map));
        }

        if req.constraints == PathConstraints::Pedestrian {
            if req.start.lane() == req.end.lane() {
                return Some(one_step_walking_path(&req, map));
            }
            let nodes = match self {
                Pathfinder::Dijkstra => dijkstra::simple_walking_path(&req, map)?,
                Pathfinder::CH(ref p) => p.simple_walking_path(&req, map)?,
            };
            let steps = walking_path_to_steps(nodes, map);
            return Some(Path::new(map, steps, req, Vec::new()));
        }

        if params != map.routing_params() {
            // If the params differ from the ones baked into the map, the CHs won't match. This
            // should only be happening from the debug UI; be very obnoxious if we start calling it
            // from the simulation or something else.
            warn!("Pathfinding slowly for {} with custom params", req);
            return dijkstra::simple_pathfind(&req, params, map);
        }

        match self {
            Pathfinder::Dijkstra => dijkstra::simple_pathfind(&req, params, map),
            Pathfinder::CH(ref p) => p.simple_pathfind(&req, map),
        }
    }

    /// Note this is a slower implementation, never using contraction hierarchies. Used for
    /// experimental congestion capping.
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

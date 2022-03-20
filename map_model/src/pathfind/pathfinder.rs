use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use abstutil::{Timer, VecMap};
use geom::Duration;

use crate::pathfind::engine::CreateEngine;
use crate::pathfind::vehicles::VehiclePathfinder;
use crate::pathfind::walking::SidewalkPathfinder;
use crate::{
    DirectedRoadID, Map, PathConstraints, PathRequest, PathV2, Position, RoutingParams,
    TransitRouteID, TransitStopID,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Pathfinder {
    car_graph: VehiclePathfinder,
    bike_graph: VehiclePathfinder,
    bus_graph: VehiclePathfinder,
    train_graph: VehiclePathfinder,
    walking_graph: SidewalkPathfinder,
    walking_with_transit_graph: SidewalkPathfinder,

    // These params cover the main graphs
    params: RoutingParams,
}

impl Pathfinder {
    /// Quickly create an invalid pathfinder, just to make borrow checking / initialization order
    /// work.
    pub fn empty() -> Pathfinder {
        Pathfinder {
            car_graph: VehiclePathfinder::empty(),
            bike_graph: VehiclePathfinder::empty(),
            bus_graph: VehiclePathfinder::empty(),
            train_graph: VehiclePathfinder::empty(),
            walking_graph: SidewalkPathfinder::empty(),
            walking_with_transit_graph: SidewalkPathfinder::empty(),
            params: RoutingParams::default(),
        }
    }

    pub(crate) fn new(
        map: &Map,
        params: RoutingParams,
        engine: &CreateEngine,
        timer: &mut Timer,
    ) -> Pathfinder {
        timer.start("prepare pathfinding for cars");
        let car_graph = VehiclePathfinder::new(map, PathConstraints::Car, &params, engine);
        timer.stop("prepare pathfinding for cars");

        // The edge weights for bikes are so different from the driving graph that reusing the node
        // ordering actually hurts!
        timer.start("prepare pathfinding for bikes");
        let bike_graph = VehiclePathfinder::new(map, PathConstraints::Bike, &params, engine);
        timer.stop("prepare pathfinding for bikes");

        timer.start("prepare pathfinding for buses");
        let bus_graph = VehiclePathfinder::new(
            map,
            PathConstraints::Bus,
            &params,
            &car_graph.engine.reuse_ordering(),
        );
        timer.stop("prepare pathfinding for buses");

        // Light rail networks are absolutely tiny; using a contraction hierarchy for them is
        // overkill. And in fact, it costs a bit of memory and file size, so don't do it!
        timer.start("prepare pathfinding for trains");
        let train_graph = VehiclePathfinder::new(
            map,
            PathConstraints::Train,
            &params,
            &CreateEngine::Dijkstra,
        );
        timer.stop("prepare pathfinding for trains");

        timer.start("prepare pathfinding for pedestrians");
        let walking_graph = SidewalkPathfinder::new(map, None, engine);
        timer.stop("prepare pathfinding for pedestrians");

        // Transit routes haven't been created yet, so defer this step
        let walking_with_transit_graph = SidewalkPathfinder::empty();

        Pathfinder {
            car_graph,
            bike_graph,
            bus_graph,
            train_graph,
            walking_graph,
            walking_with_transit_graph,
            params,
        }
    }

    /// Create a new Pathfinder with custom routing params that can only serve some modes. Fast to
    /// create, slow to use.
    pub fn new_dijkstra(
        map: &Map,
        params: RoutingParams,
        modes: Vec<PathConstraints>,
        timer: &mut Timer,
    ) -> Self {
        Self::new_limited(map, params, CreateEngine::Dijkstra, modes, timer)
    }

    /// Create a new Pathfinder with custom routing params that can only serve some modes. Slow to
    /// create, fast to use. Doesn't re-use the node ordering when building the CH.
    pub fn new_ch(
        map: &Map,
        params: RoutingParams,
        modes: Vec<PathConstraints>,
        timer: &mut Timer,
    ) -> Self {
        Self::new_limited(map, params, CreateEngine::CH, modes, timer)
    }

    /// Create a new Pathfinder with custom routing params that can only serve some modes.
    pub(crate) fn new_limited(
        map: &Map,
        params: RoutingParams,
        engine: CreateEngine,
        modes: Vec<PathConstraints>,
        timer: &mut Timer,
    ) -> Self {
        let mut p = Pathfinder::empty();
        for constraints in modes {
            timer.start(format!("prepare pathfinding for just {:?}", constraints));
            match constraints {
                PathConstraints::Pedestrian => {
                    p.walking_graph = SidewalkPathfinder::new(map, None, &engine);
                }
                PathConstraints::Car => {
                    p.car_graph = VehiclePathfinder::new(map, constraints, &params, &engine);
                }
                PathConstraints::Bike => {
                    p.bike_graph = VehiclePathfinder::new(map, constraints, &params, &engine);
                }
                PathConstraints::Bus => {
                    p.bus_graph = VehiclePathfinder::new(map, constraints, &params, &engine);
                }
                PathConstraints::Train => {
                    p.train_graph = VehiclePathfinder::new(map, constraints, &params, &engine);
                }
            }
            timer.stop(format!("prepare pathfinding for just {:?}", constraints));
        }
        p.params = params;
        p
    }

    pub fn finalize_transit(&mut self, map: &Map, engine: &CreateEngine) {
        self.walking_with_transit_graph =
            SidewalkPathfinder::new(map, Some((&self.bus_graph, &self.train_graph)), engine);
    }

    /// Finds a path from a start to an end for a certain type of agent.
    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<PathV2> {
        match req.constraints {
            PathConstraints::Pedestrian => self.walking_graph.pathfind(req, map),
            PathConstraints::Car => self.car_graph.pathfind(req, map),
            PathConstraints::Bike => self.bike_graph.pathfind(req, map),
            PathConstraints::Bus => self.bus_graph.pathfind(req, map),
            PathConstraints::Train => self.train_graph.pathfind(req, map),
        }
    }

    pub fn all_costs_from(
        &self,
        req: PathRequest,
        map: &Map,
    ) -> Option<(Duration, HashMap<DirectedRoadID, Duration>)> {
        let req_cost = self.pathfind(req.clone(), map)?.get_cost();
        let all_costs = match req.constraints {
            PathConstraints::Pedestrian => self.walking_graph.all_costs_from(req.start, map),
            PathConstraints::Car => self.car_graph.all_costs_from(req.start, map),
            PathConstraints::Bike => self.bike_graph.all_costs_from(req.start, map),
            PathConstraints::Bus | PathConstraints::Train => unreachable!(),
        };
        Some((req_cost, all_costs))
    }

    // TODO Consider returning the walking-only path in the failure case, to avoid wasting work
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(TransitStopID, Option<TransitStopID>, TransitRouteID)> {
        self.walking_with_transit_graph
            .should_use_transit(map, start, end)
    }

    pub fn apply_edits(&mut self, map: &Map, timer: &mut Timer) {
        timer.start("apply edits to car pathfinding");
        self.car_graph.apply_edits(map);
        timer.stop("apply edits to car pathfinding");

        timer.start("apply edits to bike pathfinding");
        self.bike_graph.apply_edits(map);
        timer.stop("apply edits to bike pathfinding");

        timer.start("apply edits to bus pathfinding");
        self.bus_graph.apply_edits(map);
        timer.stop("apply edits to bus pathfinding");

        timer.start("apply edits to train pathfinding");
        self.train_graph.apply_edits(map);
        timer.stop("apply edits to train pathfinding");

        timer.start("apply edits to pedestrian pathfinding");
        self.walking_graph.apply_edits(map, None);
        timer.stop("apply edits to pedestrian pathfinding");

        timer.start("apply edits to pedestrian using transit pathfinding");
        self.walking_with_transit_graph
            .apply_edits(map, Some((&self.bus_graph, &self.train_graph)));
        timer.stop("apply edits to pedestrian using transit pathfinding");
    }
}

/// For callers needing to request paths with a variety of RoutingParams. The caller is in charge
/// of the lifetime, so they can clear it out when appropriate.
pub struct PathfinderCache {
    cache: VecMap<(PathConstraints, RoutingParams), Pathfinder>,
}

impl PathfinderCache {
    pub fn new() -> Self {
        Self {
            cache: VecMap::new(),
        }
    }

    /// New pathfinders will be created as-needed using Dijkstra's, no spammy logging
    pub fn pathfind_with_params(
        &mut self,
        map: &Map,
        req: PathRequest,
        params: RoutingParams,
    ) -> Option<PathV2> {
        if let Some(pathfinder) = self.cache.get(&(req.constraints, params.clone())) {
            return pathfinder.pathfind(req, map);
        }

        let pathfinder = Pathfinder::new_limited(
            map,
            params.clone(),
            CreateEngine::Dijkstra,
            vec![req.constraints],
            &mut Timer::throwaway(),
        );
        let result = pathfinder.pathfind(req.clone(), map);
        self.cache.push((req.constraints, params), pathfinder);
        result
    }
}

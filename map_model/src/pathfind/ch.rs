//! Uses https://github.com/easbar/fast_paths. Slower creation during map importing, but very fast
//! queries.

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use geom::Duration;

use crate::pathfind::dijkstra;
use crate::pathfind::vehicles::VehiclePathfinder;
use crate::pathfind::walking::SidewalkPathfinder;
use crate::{BusRouteID, BusStopID, Map, Path, PathConstraints, PathRequest, Position};

#[derive(Serialize, Deserialize)]
pub struct ContractionHierarchyPathfinder {
    car_graph: VehiclePathfinder,
    bike_graph: VehiclePathfinder,
    bus_graph: VehiclePathfinder,
    walking_graph: SidewalkPathfinder,
    walking_with_transit_graph: SidewalkPathfinder,
}

impl ContractionHierarchyPathfinder {
    pub fn new(map: &Map, timer: &mut Timer) -> ContractionHierarchyPathfinder {
        timer.start("prepare pathfinding for cars");
        let car_graph = VehiclePathfinder::new(map, PathConstraints::Car, None);
        timer.stop("prepare pathfinding for cars");

        // The edge weights for bikes are so different from the driving graph that reusing the node
        // ordering actually hurts!
        timer.start("prepare pathfinding for bikes");
        let bike_graph = VehiclePathfinder::new(map, PathConstraints::Bike, None);
        timer.stop("prepare pathfinding for bikes");

        timer.start("prepare pathfinding for buses");
        let bus_graph = VehiclePathfinder::new(map, PathConstraints::Bus, Some(&car_graph));
        timer.stop("prepare pathfinding for buses");

        timer.start("prepare pathfinding for pedestrians");
        let walking_graph = SidewalkPathfinder::new(map, false, &bus_graph);
        timer.stop("prepare pathfinding for pedestrians");

        timer.start("prepare pathfinding for pedestrians using transit");
        let walking_with_transit_graph = SidewalkPathfinder::new(map, true, &bus_graph);
        timer.stop("prepare pathfinding for pedestrians using transit");

        ContractionHierarchyPathfinder {
            car_graph,
            bike_graph,
            bus_graph,
            walking_graph,
            walking_with_transit_graph,
        }
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<Path> {
        (match req.constraints {
            PathConstraints::Pedestrian => self.walking_graph.pathfind(req, map),
            PathConstraints::Car => self.car_graph.pathfind(req, map),
            PathConstraints::Bike => self.bike_graph.pathfind(req, map),
            PathConstraints::Bus => self.bus_graph.pathfind(req, map),
            // Light rail networks are absolutely tiny; using a contraction hierarchy for them is
            // overkill. And in fact, it costs a bit of memory and file size, so don't do it!
            PathConstraints::Train => dijkstra::pathfind(req, map.routing_params(), map),
        })
        .map(|(path, _)| path)
    }

    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
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

        timer.start("apply edits to pedestrian pathfinding");
        self.walking_graph.apply_edits(map, &self.bus_graph);
        timer.stop("apply edits to pedestrian pathfinding");

        timer.start("apply edits to pedestrian using transit pathfinding");
        self.walking_with_transit_graph
            .apply_edits(map, &self.bus_graph);
        timer.stop("apply edits to pedestrian using transit pathfinding");
    }
}

pub fn round(cost: Duration) -> usize {
    // Round up! 0 cost edges are ignored
    (cost.inner_seconds().round() as usize).max(1)
}

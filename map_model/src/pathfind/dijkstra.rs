//! Pathfinding without needing to build a separate contraction hierarchy.

use petgraph::graphmap::DiGraphMap;

use geom::Duration;

use crate::pathfind::walking::{one_step_walking_path, walking_path_to_steps, WalkingNode};
use crate::pathfind::{vehicle_cost, zone_cost};
use crate::{
    DirectedRoadID, Map, MovementID, PathConstraints, PathRequest, PathV2, RoutingParams,
    Traversable,
};

// TODO These should maybe keep the DiGraphMaps as state. It's cheap to recalculate it for edits.

pub fn pathfind(req: PathRequest, params: &RoutingParams, map: &Map) -> Option<PathV2> {
    if req.constraints == PathConstraints::Pedestrian {
        pathfind_walking(req, map)
    } else {
        let graph = build_graph_for_vehicles(map, req.constraints);
        calc_path(graph, req, params, map)
    }
}

pub fn build_graph_for_vehicles(
    map: &Map,
    constraints: PathConstraints,
) -> DiGraphMap<DirectedRoadID, MovementID> {
    let mut graph = DiGraphMap::new();
    for dr in map.all_directed_roads_for(constraints) {
        for mvmnt in map.get_movements_for(dr, constraints) {
            graph.add_edge(mvmnt.from, mvmnt.to, mvmnt);
        }
    }
    graph
}

fn calc_path(
    graph: DiGraphMap<DirectedRoadID, MovementID>,
    req: PathRequest,
    params: &RoutingParams,
    map: &Map,
) -> Option<PathV2> {
    let end = map.get_l(req.end.lane()).get_directed_parent();
    let (cost, steps) = petgraph::algo::astar(
        &graph,
        map.get_l(req.start.lane()).get_directed_parent(),
        |dr| dr == end,
        |(_, _, mvmnt)| {
            vehicle_cost(mvmnt.from, *mvmnt, req.constraints, params, map)
                + zone_cost(*mvmnt, req.constraints, map)
        },
        |_| Duration::ZERO,
    )?;
    // TODO No uber-turns yet
    Some(PathV2::from_roads(steps, req, cost, Vec::new(), map))
}

pub fn build_graph_for_pedestrians(map: &Map) -> DiGraphMap<WalkingNode, Duration> {
    let max_speed = Some(crate::MAX_WALKING_SPEED);
    let mut graph: DiGraphMap<WalkingNode, Duration> = DiGraphMap::new();
    for l in map.all_lanes().values() {
        if l.is_walkable() {
            let cost = l.length()
                / Traversable::Lane(l.id).max_speed_along(
                    max_speed,
                    PathConstraints::Pedestrian,
                    map,
                );
            let n1 = WalkingNode::SidewalkEndpoint(l.get_directed_parent(), true);
            let n2 = WalkingNode::SidewalkEndpoint(l.get_directed_parent(), false);
            graph.add_edge(n1, n2, cost);
            graph.add_edge(n2, n1, cost);

            for turn in map.get_turns_for(l.id, PathConstraints::Pedestrian) {
                graph.add_edge(
                    WalkingNode::SidewalkEndpoint(
                        l.get_directed_parent(),
                        l.dst_i == turn.id.parent,
                    ),
                    WalkingNode::SidewalkEndpoint(
                        map.get_l(turn.id.dst).get_directed_parent(),
                        map.get_l(turn.id.dst).dst_i == turn.id.parent,
                    ),
                    turn.geom.length()
                        / Traversable::Turn(turn.id).max_speed_along(
                            max_speed,
                            PathConstraints::Pedestrian,
                            map,
                        )
                        + zone_cost(turn.id.to_movement(map), PathConstraints::Pedestrian, map),
                );
            }
        }
    }
    graph
}

fn pathfind_walking(req: PathRequest, map: &Map) -> Option<PathV2> {
    if req.start.lane() == req.end.lane() {
        return Some(one_step_walking_path(req, map));
    }

    let graph = build_graph_for_pedestrians(map);

    let closest_start = WalkingNode::closest(req.start, map);
    let closest_end = WalkingNode::closest(req.end, map);
    let (cost, nodes) = petgraph::algo::astar(
        &graph,
        closest_start,
        |end| end == closest_end,
        |(_, _, cost)| *cost,
        |_| Duration::ZERO,
    )?;
    let steps = walking_path_to_steps(nodes, map);
    Some(PathV2::new(steps, req, cost, Vec::new()))
}

//! Pathfinding without needing to build a separate contraction hierarchy.

use std::collections::BTreeSet;

use petgraph::graphmap::DiGraphMap;

use geom::Duration;

use crate::pathfind::v2::path_v2_to_v1;
use crate::pathfind::vehicles::{vehicle_cost, vehicle_cost_v2};
use crate::pathfind::walking::WalkingNode;
use crate::pathfind::{zone_cost, zone_cost_v2};
use crate::{
    DirectedRoadID, LaneID, Map, MovementID, Path, PathConstraints, PathRequest, PathStep,
    RoutingParams, Traversable, TurnID,
};

// TODO These should maybe keep the DiGraphMaps as state. It's cheap to recalculate it for edits.

pub fn simple_pathfind(
    req: &PathRequest,
    params: &RoutingParams,
    map: &Map,
) -> Option<(Path, Duration)> {
    let graph = build_graph_for_vehicles_v2(map, req.constraints);
    calc_path_v2(graph, req, params, map)
}

pub fn build_graph_for_vehicles(
    map: &Map,
    constraints: PathConstraints,
) -> DiGraphMap<LaneID, TurnID> {
    let mut graph: DiGraphMap<LaneID, TurnID> = DiGraphMap::new();
    for l in map.all_lanes() {
        if constraints.can_use(l, map) {
            for turn in map.get_turns_for(l.id, constraints) {
                graph.add_edge(turn.id.src, turn.id.dst, turn.id);
            }
        }
    }
    graph
}

fn build_graph_for_vehicles_v2(
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

pub fn pathfind_avoiding_lanes(
    req: PathRequest,
    avoid: BTreeSet<LaneID>,
    map: &Map,
) -> Option<(Path, Duration)> {
    assert_eq!(req.constraints, PathConstraints::Car);
    let mut graph: DiGraphMap<LaneID, TurnID> = DiGraphMap::new();
    for l in map.all_lanes() {
        if req.constraints.can_use(l, map) && !avoid.contains(&l.id) {
            for turn in map.get_turns_for(l.id, req.constraints) {
                graph.add_edge(turn.id.src, turn.id.dst, turn.id);
            }
        }
    }

    calc_path(graph, &req, map.routing_params(), map)
}

fn calc_path(
    graph: DiGraphMap<LaneID, TurnID>,
    req: &PathRequest,
    params: &RoutingParams,
    map: &Map,
) -> Option<(Path, Duration)> {
    let (cost, path) = petgraph::algo::astar(
        &graph,
        req.start.lane(),
        |l| l == req.end.lane(),
        |(_, _, t)| {
            let turn = map.get_t(*t);
            vehicle_cost(map.get_l(turn.id.src), turn, req.constraints, params, map)
                + zone_cost(turn, req.constraints, map)
        },
        |_| Duration::ZERO,
    )?;

    let mut steps = Vec::new();
    for pair in path.windows(2) {
        steps.push(PathStep::Lane(pair[0]));
        // We don't need to look for this turn in the map; we know it exists.
        steps.push(PathStep::Turn(TurnID {
            parent: map.get_l(pair[0]).dst_i,
            src: pair[0],
            dst: pair[1],
        }));
    }
    steps.push(PathStep::Lane(req.end.lane()));
    assert_eq!(steps[0], PathStep::Lane(req.start.lane()));
    // TODO Dijkstra's for vehicles currently ignores uber-turns!
    Some((Path::new(map, steps, req.clone(), Vec::new()), cost))
}

fn calc_path_v2(
    graph: DiGraphMap<DirectedRoadID, MovementID>,
    req: &PathRequest,
    params: &RoutingParams,
    map: &Map,
) -> Option<(Path, Duration)> {
    let end = map.get_l(req.end.lane()).get_directed_parent(map);
    let (cost, path) = petgraph::algo::astar(
        &graph,
        map.get_l(req.start.lane()).get_directed_parent(map),
        |dr| dr == end,
        |(_, _, mvmnt)| {
            vehicle_cost_v2(mvmnt.from, *mvmnt, req.constraints, params, map)
                + zone_cost_v2(*mvmnt, req.constraints, map)
        },
        |_| Duration::ZERO,
    )?;

    let mut steps = Vec::new();
    for pair in path.windows(2) {
        steps.push(pair[0]);
    }
    steps.push(end);
    let path = path_v2_to_v1(req.clone(), steps, map).ok()?;
    Some((path, cost))
}

pub fn build_graph_for_pedestrians(map: &Map) -> DiGraphMap<WalkingNode, Duration> {
    let max_speed = Some(crate::MAX_WALKING_SPEED);
    let mut graph: DiGraphMap<WalkingNode, Duration> = DiGraphMap::new();
    for l in map.all_lanes() {
        if l.is_walkable() {
            let cost = l.length()
                / Traversable::Lane(l.id).max_speed_along(
                    max_speed,
                    PathConstraints::Pedestrian,
                    map,
                );
            let n1 = WalkingNode::SidewalkEndpoint(l.id, true);
            let n2 = WalkingNode::SidewalkEndpoint(l.id, false);
            graph.add_edge(n1, n2, cost);
            graph.add_edge(n2, n1, cost);

            for turn in map.get_turns_for(l.id, PathConstraints::Pedestrian) {
                graph.add_edge(
                    WalkingNode::SidewalkEndpoint(l.id, l.dst_i == turn.id.parent),
                    WalkingNode::SidewalkEndpoint(
                        turn.id.dst,
                        map.get_l(turn.id.dst).dst_i == turn.id.parent,
                    ),
                    turn.geom.length()
                        / Traversable::Turn(turn.id).max_speed_along(
                            max_speed,
                            PathConstraints::Pedestrian,
                            map,
                        )
                        + zone_cost(turn, PathConstraints::Pedestrian, map),
                );
            }
        }
    }
    graph
}

pub fn simple_walking_path(req: &PathRequest, map: &Map) -> Option<Vec<WalkingNode>> {
    let graph = build_graph_for_pedestrians(map);

    let closest_start = WalkingNode::closest(req.start, map);
    let closest_end = WalkingNode::closest(req.end, map);
    let (_, path) = petgraph::algo::astar(
        &graph,
        closest_start,
        |end| end == closest_end,
        |(_, _, cost)| *cost,
        |_| Duration::ZERO,
    )?;
    Some(path)
}

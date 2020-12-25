//! Pathfinding without needing to build a separate contraction hierarchy.

use std::collections::BTreeSet;

use petgraph::graphmap::DiGraphMap;

use crate::pathfind::driving::driving_cost;
use crate::pathfind::walking::{walking_cost, WalkingNode};
use crate::{LaneID, Map, Path, PathConstraints, PathRequest, PathStep, TurnID};

// TODO These should maybe keep the DiGraphMaps as state. It's cheap to recalculate it for edits.

pub fn simple_pathfind(req: &PathRequest, map: &Map) -> Option<Path> {
    let graph = build_graph_for_vehicles(map, req.constraints);
    calc_path(graph, req, map)
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

pub fn pathfind_avoiding_lanes(
    req: PathRequest,
    avoid: BTreeSet<LaneID>,
    map: &Map,
) -> Option<Path> {
    assert_eq!(req.constraints, PathConstraints::Car);
    let mut graph: DiGraphMap<LaneID, TurnID> = DiGraphMap::new();
    for l in map.all_lanes() {
        if req.constraints.can_use(l, map) && !avoid.contains(&l.id) {
            for turn in map.get_turns_for(l.id, req.constraints) {
                graph.add_edge(turn.id.src, turn.id.dst, turn.id);
            }
        }
    }

    calc_path(graph, &req, map)
}

fn calc_path(graph: DiGraphMap<LaneID, TurnID>, req: &PathRequest, map: &Map) -> Option<Path> {
    let (_, path) = petgraph::algo::astar(
        &graph,
        req.start.lane(),
        |l| l == req.end.lane(),
        |(_, _, turn)| driving_cost(map.get_l(turn.src), map.get_t(*turn), req.constraints, map),
        |_| 0.0,
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
    Some(Path::new(map, steps, req.clone(), Vec::new()))
}

pub fn build_graph_for_pedestrians(map: &Map) -> DiGraphMap<WalkingNode, usize> {
    let mut graph: DiGraphMap<WalkingNode, usize> = DiGraphMap::new();
    for l in map.all_lanes() {
        if l.is_walkable() {
            let cost = walking_cost(l.length());
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
                    walking_cost(turn.geom.length()),
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
        |_| 0,
    )?;
    Some(path)
}

//! Pathfinding without needing to build a separate contraction hierarchy.

use petgraph::graphmap::DiGraphMap;

use fast_paths::InputGraph;
use geom::Duration;

use crate::pathfind::vehicles::{Node, VehiclePathTranslator};
use crate::pathfind::walking::{one_step_walking_path, walking_path_to_steps, WalkingNode};
use crate::pathfind::zone_cost;
use crate::{Map, PathConstraints, PathRequest, PathStep, PathV2, RoutingParams};

// TODO These should maybe keep the DiGraphMaps as state. It's cheap to recalculate it for edits.

pub fn pathfind(req: PathRequest, params: &RoutingParams, map: &Map) -> Option<PathV2> {
    if req.constraints == PathConstraints::Pedestrian {
        pathfind_walking(req, map)
    } else {
        assert!(!map.get_l(req.start.lane()).is_walkable());

        let translator = VehiclePathTranslator::new(map, req.constraints);
        let input_graph = translator.make_input_graph(params, map);
        let graph = fast_paths_to_petgraph(input_graph);

        // TODO Handle multiple starts here?

        let start = translator.nodes.get(Node::Road(
            map.get_l(req.start.lane()).get_directed_parent(),
        ));
        let end = translator
            .nodes
            .get(Node::Road(map.get_l(req.end.lane()).get_directed_parent()));
        let (raw_cost, raw_nodes) = petgraph::algo::astar(
            &graph,
            start,
            |node| node == end,
            |(_, _, cost)| *cost,
            |_| 0,
        )?;
        Some(translator.reconstruct_path(&raw_nodes, raw_cost, req, map))
    }
}

pub fn fast_paths_to_petgraph(input_graph: InputGraph) -> DiGraphMap<usize, usize> {
    let mut graph = DiGraphMap::new();
    for edge in input_graph.get_edges() {
        graph.add_edge(edge.from, edge.to, edge.weight);
    }
    graph
}

pub fn build_graph_for_pedestrians(map: &Map) -> DiGraphMap<WalkingNode, Duration> {
    let max_speed = Some(crate::MAX_WALKING_SPEED);
    let mut graph: DiGraphMap<WalkingNode, Duration> = DiGraphMap::new();
    for l in map.all_lanes().values() {
        if l.is_walkable() {
            let n1 = WalkingNode::SidewalkEndpoint(l.get_directed_parent(), false);
            let n2 = WalkingNode::SidewalkEndpoint(l.get_directed_parent(), true);
            for (step, pair) in [
                (PathStep::Lane(l.id), (n1, n2)),
                (PathStep::ContraflowLane(l.id), (n2, n1)),
            ] {
                let cost =
                    l.length() / step.max_speed_along(max_speed, PathConstraints::Pedestrian, map);
                graph.add_edge(pair.0, pair.1, cost);
            }

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
                        / PathStep::Turn(turn.id).max_speed_along(
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

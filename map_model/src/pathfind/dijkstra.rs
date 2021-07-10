//! Pathfinding without needing to build a separate contraction hierarchy.

use petgraph::graphmap::DiGraphMap;

use fast_paths::InputGraph;

use crate::pathfind::vehicles::{Node, VehiclePathTranslator};
use crate::pathfind::walking::{one_step_walking_path, SidewalkPathTranslator, WalkingNode};
use crate::{Map, PathConstraints, PathRequest, PathV2, RoutingParams};

// TODO These should maybe keep the DiGraphMaps as state. It's cheap to recalculate it for edits.

pub fn pathfind(req: PathRequest, params: &RoutingParams, map: &Map) -> Option<PathV2> {
    if req.constraints == PathConstraints::Pedestrian {
        if req.start.lane() == req.end.lane() {
            return Some(one_step_walking_path(req, map));
        }

        let translator = SidewalkPathTranslator::just_walking(map);
        let input_graph = translator.make_input_graph(map, None);
        let graph = fast_paths_to_petgraph(input_graph);

        let start = translator.nodes.get(WalkingNode::closest(req.start, map));
        let end = translator.nodes.get(WalkingNode::closest(req.end, map));
        let (raw_cost, raw_nodes) = petgraph::algo::astar(
            &graph,
            start,
            |node| node == end,
            |(_, _, cost)| *cost,
            |_| 0,
        )?;
        Some(translator.reconstruct_path(&raw_nodes, raw_cost, req, map))
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

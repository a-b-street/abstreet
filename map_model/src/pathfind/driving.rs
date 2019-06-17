use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{LaneID, LaneType, Map, Path, PathRequest, PathStep, TurnID};
use fast_paths::{FastGraph, InputGraph};
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VehiclePathfinder {
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<LaneID>,
    lane_types: Vec<LaneType>,
}

impl VehiclePathfinder {
    pub fn new(map: &Map, lane_types: Vec<LaneType>) -> VehiclePathfinder {
        let mut input_graph = InputGraph::new();
        let mut nodes = NodeMap::new();

        for l in map.all_lanes() {
            // Insert every lane as a node. Even if the lane type is wrong now, it might change
            // later, and we want the node in the graph.
            let from = nodes.get_or_insert(l.id);

            for (turn, next) in map.get_next_turns_and_lanes(l.id, l.dst_i).into_iter() {
                if !map.is_turn_allowed(turn.id) || !lane_types.contains(&next.lane_type) {
                    continue;
                }
                // TODO Speed limit or some other cost
                let length = l.length() + turn.geom.length();
                let length_cm = (length.inner_meters() * 100.0).round() as usize;
                input_graph.add_edge(from, nodes.get_or_insert(next.id), length_cm);
            }
        }
        input_graph.freeze();
        let graph = fast_paths::prepare(&input_graph);

        VehiclePathfinder {
            graph,
            nodes,
            lane_types,
        }
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<Path> {
        assert!(!map.get_l(req.start.lane()).is_sidewalk());
        let raw_path = fast_paths::calc_path(
            &self.graph,
            self.nodes.get(req.start.lane()),
            self.nodes.get(req.end.lane()),
        )?;
        let mut steps = Vec::new();
        for pair in self.nodes.translate(&raw_path).windows(2) {
            steps.push(PathStep::Lane(pair[0]));
            // We don't need to look for this turn in the map; we know it exists.
            steps.push(PathStep::Turn(TurnID {
                parent: map.get_l(pair[0]).dst_i,
                src: pair[0],
                dst: pair[1],
            }));
        }
        steps.push(PathStep::Lane(req.end.lane()));
        Some(Path::new(map, steps, req.end.dist_along()))
    }

    pub fn apply_edits(&mut self, map: &Map) {
        // The NodeMap is just all lanes -- it won't change. So we can also reuse the node
        // ordering.
        // TODO Make sure the result of this is deterministic and equivalent to computing from
        // scratch.
        let mut input_graph = InputGraph::new();

        for l in map.all_lanes() {
            for (turn, next) in map.get_next_turns_and_lanes(l.id, l.dst_i).into_iter() {
                if !map.is_turn_allowed(turn.id) || !self.lane_types.contains(&next.lane_type) {
                    continue;
                }
                // TODO Speed limit or some other cost
                let length = l.length() + turn.geom.length();
                let length_cm = (length.inner_meters() * 100.0).round() as usize;
                input_graph.add_edge(self.nodes.get(l.id), self.nodes.get(next.id), length_cm);
            }
        }
        input_graph.freeze();
        let node_ordering = self.graph.get_node_ordering();
        self.graph = fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap();
    }
}

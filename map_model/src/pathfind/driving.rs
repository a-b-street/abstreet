use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{LaneID, LaneType, Map, Path, PathRequest, PathStep, TurnID};
use fast_paths::{FastGraph, InputGraph, PathCalculator};
use geom::Distance;
use serde_derive::{Deserialize, Serialize};
use std::cell::RefCell;
use thread_local::ThreadLocal;

#[derive(Serialize, Deserialize)]
pub struct VehiclePathfinder {
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<LaneID>,
    lane_types: Vec<LaneType>,

    #[serde(skip_serializing, skip_deserializing)]
    path_calc: ThreadLocal<RefCell<PathCalculator>>,
}

impl VehiclePathfinder {
    pub fn new(
        map: &Map,
        lane_types: Vec<LaneType>,
        seed: Option<&VehiclePathfinder>,
    ) -> VehiclePathfinder {
        // Insert every lane as a node. Even if the lane type is wrong now, it might change later,
        // and we want the node in the graph. Do this first, so the IDs of all the nodes doesn't
        // depend on lane types and turns and such.
        let mut nodes = NodeMap::new();
        for l in map.all_lanes() {
            nodes.get_or_insert(l.id);
        }
        let input_graph = make_input_graph(map, &nodes, &lane_types);

        // All VehiclePathfinders have the same nodes (lanes), so if we're not the first being
        // built, seed from the node ordering.
        let graph = if let Some(seed) = seed {
            let node_ordering = seed.graph.get_node_ordering();
            fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap()
        } else {
            fast_paths::prepare(&input_graph)
        };

        VehiclePathfinder {
            graph,
            nodes,
            lane_types,
            path_calc: ThreadLocal::new(),
        }
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<Path> {
        assert!(!map.get_l(req.start.lane()).is_sidewalk());
        let mut calc = self
            .path_calc
            .get_or(|| Box::new(RefCell::new(fast_paths::create_calculator(&self.graph))))
            .borrow_mut();
        let raw_path = calc.calc_path(
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
        Some(Path::new(
            map,
            steps,
            req.end.dist_along(),
            Distance::centimeters(raw_path.get_weight()),
        ))
    }

    pub fn apply_edits(&mut self, map: &Map) {
        // The NodeMap is just all lanes -- it won't change. So we can also reuse the node
        // ordering.
        // TODO Make sure the result of this is deterministic and equivalent to computing from
        // scratch.
        let input_graph = make_input_graph(map, &self.nodes, &self.lane_types);
        let node_ordering = self.graph.get_node_ordering();
        self.graph = fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap();
    }
}

fn make_input_graph(map: &Map, nodes: &NodeMap<LaneID>, lane_types: &Vec<LaneType>) -> InputGraph {
    let mut input_graph = InputGraph::new();
    let num_lanes = map.all_lanes().len();
    for l in map.all_lanes() {
        let from = nodes.get(l.id);
        let mut any = false;
        for (turn, next) in map.get_next_turns_and_lanes(l.id, l.dst_i).into_iter() {
            if !map.is_turn_allowed(turn.id) || !lane_types.contains(&next.lane_type) {
                continue;
            }
            any = true;
            // TODO Speed limit or some other cost
            let length = l.length() + turn.geom.length();
            let length_cm = (length.inner_meters() * 100.0).round() as usize;
            input_graph.add_edge(from, nodes.get(next.id), length_cm);
        }
        // The nodes in the graph MUST exactly be all of the lanes, so we can reuse node
        // ordering later. If the last lane doesn't have any edges, then this won't work. So
        // pretend like it points to some arbitrary other node. Since no paths will start from
        // this unused node, this won't affect results.
        // TODO Upstream a method in InputGraph to do this more clearly.
        if !any && l.id.0 == num_lanes - 1 {
            input_graph.add_edge(from, nodes.get(LaneID(0)), 1);
        }
    }
    input_graph.freeze();
    input_graph
}

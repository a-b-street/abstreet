use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{Lane, LaneID, Map, Path, PathConstraints, PathRequest, PathStep, Turn, TurnID};
use fast_paths::{FastGraph, InputGraph, PathCalculator};
use serde_derive::{Deserialize, Serialize};
use std::cell::RefCell;
use thread_local::ThreadLocal;

#[derive(Serialize, Deserialize)]
pub struct VehiclePathfinder {
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<LaneID>,
    constraints: PathConstraints,

    #[serde(skip_serializing, skip_deserializing)]
    path_calc: ThreadLocal<RefCell<PathCalculator>>,
}

impl VehiclePathfinder {
    pub fn new(
        map: &Map,
        constraints: PathConstraints,
        seed: Option<&VehiclePathfinder>,
    ) -> VehiclePathfinder {
        // Insert every lane as a node. Even if the lane type is wrong now, it might change later,
        // and we want the node in the graph. Do this first, so the IDs of all the nodes doesn't
        // depend on lane types and turns and such.
        let mut nodes = NodeMap::new();
        for l in map.all_lanes() {
            nodes.get_or_insert(l.id);
        }
        let input_graph = make_input_graph(map, &nodes, constraints);

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
            constraints,
            path_calc: ThreadLocal::new(),
        }
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<(Path, usize)> {
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
        Some((
            Path::new(map, steps, req.end.dist_along()),
            raw_path.get_weight(),
        ))
    }

    pub fn apply_edits(&mut self, map: &Map) {
        // The NodeMap is just all lanes -- it won't change. So we can also reuse the node
        // ordering.
        // TODO Make sure the result of this is deterministic and equivalent to computing from
        // scratch.
        let input_graph = make_input_graph(map, &self.nodes, self.constraints);
        let node_ordering = self.graph.get_node_ordering();
        self.graph = fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap();
    }
}

fn make_input_graph(
    map: &Map,
    nodes: &NodeMap<LaneID>,
    constraints: PathConstraints,
) -> InputGraph {
    let mut input_graph = InputGraph::new();
    let num_lanes = map.all_lanes().len();
    for l in map.all_lanes() {
        let from = nodes.get(l.id);
        let mut any = false;
        if constraints.can_use(l, map) {
            for turn in map.get_turns_for(l.id, constraints) {
                any = true;
                input_graph.add_edge(
                    from,
                    nodes.get(turn.id.dst),
                    cost(l, turn, constraints, map),
                );
            }
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

pub fn cost(lane: &Lane, turn: &Turn, constraints: PathConstraints, map: &Map) -> usize {
    // TODO Could cost turns differently.

    match constraints {
        PathConstraints::Car => {
            // Prefer slightly longer route on faster roads
            let t1 = lane.length() / map.get_r(lane.parent).get_speed_limit();
            let t2 = turn.geom.length() / map.get_parent(turn.id.dst).get_speed_limit();
            (t1 + t2).inner_seconds().round() as usize
        }
        PathConstraints::Bike => {
            // Speed limits don't matter, bikes are usually constrained by their own speed limit.
            let dist = lane.length() + turn.geom.length();
            // TODO Elevation gain is bad, loss is good.
            // TODO If we're on a driving lane, higher speed limit is worse.
            // TODO Bike lanes next to parking is dangerous.

            // TODO Prefer bike lanes, then bus lanes, then driving lanes. For now, express that as
            // an extra cost.
            let lt_penalty = if lane.is_biking() {
                1.0
            } else if lane.is_bus() {
                1.1
            } else {
                assert!(lane.is_driving());
                1.5
            };

            // 1m resolution is fine
            (lt_penalty * dist).inner_meters().round() as usize
        }
        PathConstraints::Bus => {
            // Like Car, but prefer bus lanes.
            let t1 = lane.length() / map.get_r(lane.parent).get_speed_limit();
            let t2 = turn.geom.length() / map.get_parent(turn.id.dst).get_speed_limit();
            let lt_penalty = if lane.is_bus() {
                1.0
            } else {
                assert!(lane.is_driving());
                1.1
            };
            (lt_penalty * (t1 + t2)).inner_seconds().round() as usize
        }
        PathConstraints::Pedestrian => unreachable!(),
    }
}

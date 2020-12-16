//! Pathfinding for cars, bikes, buses, and trains using contraction hierarchies

use std::cell::RefCell;

use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use serde::{Deserialize, Serialize};
use thread_local::ThreadLocal;

use abstutil::MultiMap;

use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::pathfind::uber_turns::{IntersectionCluster, UberTurn};
use crate::{Lane, LaneID, Map, Path, PathConstraints, PathRequest, PathStep, Turn, TurnID};

#[derive(Serialize, Deserialize)]
pub struct VehiclePathfinder {
    #[serde(serialize_with = "serialize_32", deserialize_with = "deserialize_32")]
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<Node>,
    uber_turns: Vec<UberTurn>,
    constraints: PathConstraints,

    #[serde(skip_serializing, skip_deserializing)]
    path_calc: ThreadLocal<RefCell<PathCalculator>>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
enum Node {
    Lane(LaneID),
    UberTurn(usize),
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
            nodes.get_or_insert(Node::Lane(l.id));
        }

        // Find all uber-turns and make a node for them too.
        let mut uber_turns = Vec::new();
        for ic in IntersectionCluster::find_all(map) {
            for ut in ic.uber_turns {
                nodes.get_or_insert(Node::UberTurn(uber_turns.len()));
                uber_turns.push(ut);
            }
        }

        let input_graph = make_input_graph(map, &nodes, &uber_turns, constraints);

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
            uber_turns,
            constraints,
            path_calc: ThreadLocal::new(),
        }
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<(Path, usize)> {
        assert!(!map.get_l(req.start.lane()).is_walkable());
        let mut calc = self
            .path_calc
            .get_or(|| RefCell::new(fast_paths::create_calculator(&self.graph)))
            .borrow_mut();
        let raw_path = calc.calc_path(
            &self.graph,
            self.nodes.get(Node::Lane(req.start.lane())),
            self.nodes.get(Node::Lane(req.end.lane())),
        )?;
        let mut steps = Vec::new();
        let mut uber_turns = Vec::new();
        for pair in self.nodes.translate(&raw_path).windows(2) {
            match (pair[0], pair[1]) {
                (Node::Lane(l1), Node::Lane(l2)) => {
                    steps.push(PathStep::Lane(l1));
                    // We don't need to look for this turn in the map; we know it exists.
                    steps.push(PathStep::Turn(TurnID {
                        parent: map.get_l(l1).dst_i,
                        src: l1,
                        dst: l2,
                    }));
                }
                (Node::Lane(l), Node::UberTurn(ut)) => {
                    steps.push(PathStep::Lane(l));
                    let ut = self.uber_turns[ut].clone();
                    for t in &ut.path {
                        steps.push(PathStep::Turn(*t));
                        steps.push(PathStep::Lane(t.dst));
                    }
                    steps.pop();
                    uber_turns.push(ut);
                }
                (Node::UberTurn(_), Node::Lane(_)) => {
                    // Don't add anything; the lane will be added by some other case
                }
                (Node::UberTurn(_), Node::UberTurn(_)) => unreachable!(),
            }
        }
        steps.push(PathStep::Lane(req.end.lane()));
        Some((
            Path::new(map, steps, req.clone(), uber_turns),
            raw_path.get_weight(),
        ))
    }

    pub fn apply_edits(&mut self, map: &Map) {
        // The NodeMap is just all lanes and uber-turns -- it won't change. So we can also reuse
        // the node ordering.
        // TODO Make sure the result of this is deterministic and equivalent to computing from
        // scratch.
        let input_graph = make_input_graph(map, &self.nodes, &self.uber_turns, self.constraints);
        let node_ordering = self.graph.get_node_ordering();
        self.graph = fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap();
    }
}

fn make_input_graph(
    map: &Map,
    nodes: &NodeMap<Node>,
    uber_turns: &Vec<UberTurn>,
    constraints: PathConstraints,
) -> InputGraph {
    let mut input_graph = InputGraph::new();

    // From some lanes, instead of adding edges to turns, add edges to these (indexed) uber-turns.
    let mut uber_turn_entrances: MultiMap<LaneID, usize> = MultiMap::new();
    for (idx, ut) in uber_turns.iter().enumerate() {
        // But actually, make sure this uber-turn only contains lanes that can be used by this
        // vehicle.
        // TODO Need to test editing lanes inside an IntersectionCluster very carefully. See Mercer
        // and Dexter.
        if ut
            .path
            .iter()
            .all(|t| constraints.can_use(map.get_l(t.dst), map))
        {
            uber_turn_entrances.insert(ut.entry(), idx);
        } else {
            // Similar to the hack below for unused lanes
            if idx == uber_turns.len() - 1 {
                input_graph.add_edge(
                    nodes.get(Node::UberTurn(idx)),
                    nodes.get(Node::UberTurn(0)),
                    1,
                );
            }
        }
    }

    let num_lanes = map.all_lanes().len();
    for l in map.all_lanes() {
        let from = nodes.get(Node::Lane(l.id));
        let mut any = false;
        if constraints.can_use(l, map)
            && map
                .get_r(l.parent)
                .access_restrictions
                .allow_through_traffic
                .contains(constraints)
        {
            let indices = uber_turn_entrances.get(l.id);
            if indices.is_empty() {
                for turn in map.get_turns_for(l.id, constraints) {
                    any = true;
                    input_graph.add_edge(
                        from,
                        nodes.get(Node::Lane(turn.id.dst)),
                        round(driving_cost(l, turn, constraints, map)),
                    );
                }
            } else {
                for idx in indices {
                    any = true;
                    let ut = &uber_turns[*idx];

                    let mut sum_cost = 0.0;
                    for t in &ut.path {
                        sum_cost += driving_cost(map.get_l(t.src), map.get_t(*t), constraints, map);
                    }
                    input_graph.add_edge(from, nodes.get(Node::UberTurn(*idx)), round(sum_cost));
                    input_graph.add_edge(
                        nodes.get(Node::UberTurn(*idx)),
                        nodes.get(Node::Lane(ut.exit())),
                        // The cost is already captured for entering the uber-turn
                        1,
                    );
                }
            }
        }
        // The nodes in the graph MUST exactly be all of the lanes, so we can reuse node
        // ordering later. If the last lane doesn't have any edges, then this won't work. So
        // pretend like it points to some arbitrary other node. Since no paths will start from
        // this unused node, this won't affect results.
        // TODO Upstream a method in InputGraph to do this more clearly.
        if !any && l.id.0 == num_lanes - 1 {
            input_graph.add_edge(from, nodes.get(Node::Lane(LaneID(0))), 1);
        }
    }
    input_graph.freeze();
    input_graph
}

/// Different unit based on constraints.
pub fn driving_cost(lane: &Lane, turn: &Turn, constraints: PathConstraints, map: &Map) -> f64 {
    // TODO Could cost turns differently.

    let base = match constraints {
        PathConstraints::Car | PathConstraints::Train => {
            // Prefer slightly longer route on faster roads
            let t1 = lane.length() / map.get_r(lane.parent).speed_limit;
            let t2 = turn.geom.length() / map.get_parent(turn.id.dst).speed_limit;
            (t1 + t2).inner_seconds()
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
            (lt_penalty * dist).inner_meters()
        }
        PathConstraints::Bus => {
            // Like Car, but prefer bus lanes.
            let t1 = lane.length() / map.get_r(lane.parent).speed_limit;
            let t2 = turn.geom.length() / map.get_parent(turn.id.dst).speed_limit;
            let lt_penalty = if lane.is_bus() {
                1.0
            } else {
                assert!(lane.is_driving());
                1.1
            };
            (lt_penalty * (t1 + t2)).inner_seconds()
        }
        PathConstraints::Pedestrian => unreachable!(),
    };

    // Normally opportunistic lane-changing adjusts the path live, but that doesn't work near
    // uber-turns. So still use some of the penalties here.
    let (lt, lc, slow_lane) = turn.penalty(map);
    // TODO Since these costs wind up mattering most for particular lane choice, I guess just
    // adding is reasonable?
    let mut extra_penalty = lt + lc;
    if constraints == PathConstraints::Bike {
        extra_penalty = slow_lane;
    }

    base + (extra_penalty as f64)
}

// Round up! 0 cost edges are ignored
fn round(cost: f64) -> usize {
    (cost.round() as usize).max(1)
}

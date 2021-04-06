//! Pathfinding for cars, bikes, buses, and trains using contraction hierarchies

use std::cell::RefCell;

use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use serde::{Deserialize, Serialize};
use thread_local::ThreadLocal;

use abstutil::MultiMap;
use geom::Duration;

use crate::pathfind::ch::round;
use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::pathfind::uber_turns::{IntersectionCluster, UberTurn};
use crate::pathfind::zone_cost;
use crate::{
    DirectedRoadID, DrivingSide, Lane, LaneID, LaneType, Map, MovementID, Path, PathConstraints,
    PathRequest, PathStep, RoutingParams, Traversable, Turn, TurnID, TurnType,
};

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
        // Force the nodes to always match up in the graph for different vehicle types.
        nodes.get(Node::UberTurn(idx));

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
        }
    }

    let num_lanes = map.all_lanes().len();
    let mut used_last_uber_turn = false;
    for l in map.all_lanes() {
        let from = nodes.get(Node::Lane(l.id));
        let mut any = false;
        if constraints.can_use(l, map) {
            let indices = uber_turn_entrances.get(l.id);
            if indices.is_empty() {
                for turn in map.get_turns_for(l.id, constraints) {
                    any = true;
                    input_graph.add_edge(
                        from,
                        nodes.get(Node::Lane(turn.id.dst)),
                        round(
                            vehicle_cost(l, turn, constraints, map.routing_params(), map)
                                + zone_cost(turn, constraints, map),
                        ),
                    );
                }
            } else {
                for idx in indices {
                    any = true;
                    let ut = &uber_turns[*idx];

                    let mut sum_cost = Duration::ZERO;
                    for t in &ut.path {
                        let turn = map.get_t(*t);
                        sum_cost += vehicle_cost(
                            map.get_l(t.src),
                            turn,
                            constraints,
                            map.routing_params(),
                            map,
                        ) + zone_cost(turn, constraints, map);
                    }
                    input_graph.add_edge(from, nodes.get(Node::UberTurn(*idx)), round(sum_cost));
                    input_graph.add_edge(
                        nodes.get(Node::UberTurn(*idx)),
                        nodes.get(Node::Lane(ut.exit())),
                        // The cost is already captured for entering the uber-turn
                        1,
                    );
                    if *idx == uber_turns.len() - 1 {
                        used_last_uber_turn = true;
                    }
                }
            }
        }
        // The nodes in the graph MUST exactly be all of the lanes, so we can reuse node ordering
        // later. If the last lane doesn't have any edges, then this won't work -- fast_paths trims
        // out unused nodes at the end. So pretend like it points to some arbitrary other node.
        // Since no paths will start from this unused node, this won't affect results.
        // TODO Upstream a method in InputGraph to do this more clearly.
        if !any && l.id.0 == num_lanes - 1 {
            input_graph.add_edge(from, nodes.get(Node::Lane(LaneID(0))), 1);
        }
    }

    // Same as the hack above for unused lanes
    if !used_last_uber_turn && !uber_turns.is_empty() {
        input_graph.add_edge(
            nodes.get(Node::UberTurn(uber_turns.len() - 1)),
            nodes.get(Node::UberTurn(0)),
            1,
        );
    }

    input_graph.freeze();
    input_graph
}

/// This returns the pathfinding cost of crossing one lane and turn. This is also expressed in
/// units of time. It factors in the ideal time to cross the space, along with penalties for
/// entering an access-restricted zone, taking an unprotected turn, and so on.
pub fn vehicle_cost(
    lane: &Lane,
    turn: &Turn,
    constraints: PathConstraints,
    params: &RoutingParams,
    map: &Map,
) -> Duration {
    let max_speed = match constraints {
        PathConstraints::Car | PathConstraints::Bus | PathConstraints::Train => None,
        PathConstraints::Bike => Some(crate::MAX_BIKE_SPEED),
        PathConstraints::Pedestrian => unreachable!(),
    };
    let t1 =
        lane.length() / Traversable::Lane(lane.id).max_speed_along(max_speed, constraints, map);
    let t2 = turn.geom.length()
        / Traversable::Turn(turn.id).max_speed_along(max_speed, constraints, map);

    let base = match constraints {
        PathConstraints::Car | PathConstraints::Train => t1 + t2,
        PathConstraints::Bike => {
            // TODO If we're on a driving lane, higher speed limit is worse.
            // TODO Bike lanes next to parking is dangerous.

            // TODO Prefer bike lanes, then bus lanes, then driving lanes. For now, express that by
            // multiplying the base cost.
            let lt_penalty = if lane.is_biking() {
                params.bike_lane_penalty
            } else if lane.is_bus() {
                params.bus_lane_penalty
            } else {
                assert!(lane.is_driving());
                params.driving_lane_penalty
            };

            lt_penalty * (t1 + t2)
        }
        PathConstraints::Bus => {
            // Like Car, but prefer bus lanes.
            let lt_penalty = if lane.is_bus() {
                1.0
            } else {
                assert!(lane.is_driving());
                1.1
            };
            lt_penalty * (t1 + t2)
        }
        PathConstraints::Pedestrian => unreachable!(),
    };

    // Penalize unprotected turns at a stop sign from smaller to larger roads.
    let unprotected_turn_type = if map.get_config().driving_side == DrivingSide::Right {
        TurnType::Left
    } else {
        TurnType::Right
    };
    let rank_from = map.get_r(lane.parent).get_detailed_rank();
    let rank_to = map.get_parent(turn.id.dst).get_detailed_rank();
    let base = if turn.turn_type == unprotected_turn_type
        && rank_from < rank_to
        && map.get_i(turn.id.parent).is_stop_sign()
    {
        base + params.unprotected_turn_penalty
    } else {
        base
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
    // TODO These are small integers, just treat them as seconds for now to micro-adjust the
    // specific choice of lane.

    base + Duration::seconds(extra_penalty as f64)
}

/// This returns the pathfinding cost of crossing one road and turn. This is also expressed in
/// units of time. It factors in the ideal time to cross the space, along with penalties for
/// entering an access-restricted zone, taking an unprotected turn, and so on.
// TODO Remove vehicle_cost after pathfinding v2 transition is done.
pub fn vehicle_cost_v2(
    dr: DirectedRoadID,
    mvmnt: MovementID,
    constraints: PathConstraints,
    params: &RoutingParams,
    map: &Map,
) -> Duration {
    let mvmnt = mvmnt.get(map).unwrap();
    let max_speed = match constraints {
        PathConstraints::Car | PathConstraints::Bus | PathConstraints::Train => None,
        PathConstraints::Bike => Some(crate::MAX_BIKE_SPEED),
        PathConstraints::Pedestrian => unreachable!(),
    };
    let t1 = map.get_r(dr.id).center_pts.length()
        / Traversable::max_speed_along_road(dr, max_speed, constraints, map);
    let t2 = mvmnt.geom.length()
        / Traversable::max_speed_along_movement(mvmnt.id, max_speed, constraints, map);

    let base = match constraints {
        PathConstraints::Car | PathConstraints::Train => t1 + t2,
        PathConstraints::Bike => {
            // TODO If we're on a driving lane, higher speed limit is worse.
            // TODO Bike lanes next to parking is dangerous.

            // TODO Prefer bike lanes, then bus lanes, then driving lanes. For now, express that by
            // multiplying the base cost.
            let lt_penalty = if dr.has_lanes(LaneType::Biking, map) {
                params.bike_lane_penalty
            } else if dr.has_lanes(LaneType::Bus, map) {
                params.bus_lane_penalty
            } else {
                params.driving_lane_penalty
            };

            lt_penalty * (t1 + t2)
        }
        PathConstraints::Bus => {
            // Like Car, but prefer bus lanes.
            let lt_penalty = if dr.has_lanes(LaneType::Bus, map) {
                1.0
            } else {
                1.1
            };
            lt_penalty * (t1 + t2)
        }
        PathConstraints::Pedestrian => unreachable!(),
    };

    // Penalize unprotected turns at a stop sign from smaller to larger roads.
    let unprotected_turn_type = if map.get_config().driving_side == DrivingSide::Right {
        TurnType::Left
    } else {
        TurnType::Right
    };
    let rank_from = map.get_r(dr.id).get_detailed_rank();
    let rank_to = map.get_r(mvmnt.id.to.id).get_detailed_rank();
    if mvmnt.turn_type == unprotected_turn_type
        && rank_from < rank_to
        && map.get_i(mvmnt.id.parent).is_stop_sign()
    {
        base + params.unprotected_turn_penalty
    } else {
        base
    }
}

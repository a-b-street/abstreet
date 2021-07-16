//! Pathfinding for cars, bikes, buses, and trains using contraction hierarchies

use std::collections::HashMap;

use fast_paths::InputGraph;
use serde::{Deserialize, Serialize};

use abstutil::MultiMap;
use geom::{Distance, Duration};

use crate::pathfind::engine::{CreateEngine, PathfindEngine};
use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::pathfind::uber_turns::{IntersectionCluster, UberTurnV2};
use crate::pathfind::zone_cost;
use crate::pathfind::{round, unround};
use crate::{
    DirectedRoadID, Direction, DrivingSide, LaneType, Map, MovementID, PathConstraints,
    PathRequest, PathV2, Position, RoutingParams, Traversable, TurnType,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct VehiclePathfinder {
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<Node>,
    uber_turns: Vec<UberTurnV2>,
    constraints: PathConstraints,
    params: RoutingParams,
    pub engine: PathfindEngine,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum Node {
    Road(DirectedRoadID),
    UberTurn(usize),
}

impl VehiclePathfinder {
    pub fn empty() -> VehiclePathfinder {
        VehiclePathfinder {
            nodes: NodeMap::new(),
            uber_turns: Vec::new(),
            constraints: PathConstraints::Car,
            params: RoutingParams::default(),
            engine: PathfindEngine::Empty,
        }
    }

    pub fn new(
        map: &Map,
        constraints: PathConstraints,
        params: &RoutingParams,
        engine: &CreateEngine,
    ) -> VehiclePathfinder {
        // Insert every road as a node.
        let mut nodes = NodeMap::new();
        for r in map.all_roads() {
            // Regardless of current lane types or even directions, add both. These could change
            // later, and we want the node IDs to match up.
            nodes.get_or_insert(Node::Road(DirectedRoadID {
                id: r.id,
                dir: Direction::Fwd,
            }));
            nodes.get_or_insert(Node::Road(DirectedRoadID {
                id: r.id,
                dir: Direction::Back,
            }));
        }

        // Find all uber-turns and make a node for them too.
        let mut uber_turns = Vec::new();
        for ic in IntersectionCluster::find_all(map) {
            for ut in ic.into_v2(map) {
                nodes.get_or_insert(Node::UberTurn(uber_turns.len()));
                uber_turns.push(ut);
            }
        }

        let input_graph = make_input_graph(constraints, &nodes, &uber_turns, params, map);
        let engine = engine.create(input_graph);

        VehiclePathfinder {
            nodes,
            uber_turns,
            constraints,
            params: params.clone(),
            engine,
        }
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<PathV2> {
        assert!(!map.get_l(req.start.lane()).is_walkable());
        let mut starts = vec![(
            self.nodes.get(Node::Road(
                map.get_l(req.start.lane()).get_directed_parent(),
            )),
            0,
        )];
        if let Some((pos, cost)) = req.alt_start {
            starts.push((
                self.nodes
                    .get(Node::Road(map.get_l(pos.lane()).get_directed_parent())),
                round(cost),
            ));
        }
        let (raw_weight, raw_nodes) = self.engine.calculate_path_multiple_sources_and_targets(
            starts,
            vec![(
                self.nodes
                    .get(Node::Road(map.get_l(req.end.lane()).get_directed_parent())),
                0,
            )],
        )?;

        let mut road_steps = Vec::new();
        let mut uber_turns = Vec::new();
        for node in raw_nodes.into_iter().map(|id| self.nodes.translate_id(id)) {
            match node {
                Node::Road(dr) => {
                    road_steps.push(dr);
                }
                Node::UberTurn(ut) => {
                    // Flatten the uber-turn into the roads it crosses.
                    for mvmnt in &self.uber_turns[ut].path {
                        road_steps.push(mvmnt.to);
                    }
                    road_steps.pop();
                    // Also remember the uber-turn exists.
                    uber_turns.push(self.uber_turns[ut].clone());
                }
            }
        }
        let cost = unround(raw_weight);
        Some(PathV2::from_roads(road_steps, req, cost, uber_turns, map))
    }

    pub fn apply_edits(&mut self, map: &Map) {
        // The NodeMap is just all roads and uber-turns -- it won't change. So we can also reuse
        // the node ordering.
        // TODO Make sure the result of this is deterministic and equivalent to computing from
        // scratch.
        let input_graph = make_input_graph(
            self.constraints,
            &self.nodes,
            &self.uber_turns,
            &self.params,
            map,
        );
        let engine = self.engine.reuse_ordering().create(input_graph);
        self.engine = engine;
    }

    pub fn all_costs_from(&self, start: Position, map: &Map) -> HashMap<DirectedRoadID, Duration> {
        let start = self
            .nodes
            .get(Node::Road(map.get_l(start.lane()).get_directed_parent()));
        let raw_costs = if self.engine.is_dijkstra() {
            self.engine.all_costs_from(start)
        } else {
            // The CH engine doesn't support this!
            let input_graph = make_input_graph(
                self.constraints,
                &self.nodes,
                &self.uber_turns,
                &self.params,
                map,
            );
            CreateEngine::Dijkstra
                .create(input_graph)
                .all_costs_from(start)
        };
        raw_costs
            .into_iter()
            .filter_map(|(k, v)| {
                if let Node::Road(dr) = self.nodes.translate_id(k) {
                    Some((dr, unround(v)))
                } else {
                    None
                }
            })
            .collect()
    }
}

fn make_input_graph(
    constraints: PathConstraints,
    nodes: &NodeMap<Node>,
    uber_turns: &[UberTurnV2],
    params: &RoutingParams,
    map: &Map,
) -> InputGraph {
    let mut input_graph = InputGraph::new();

    // From some roads, instead of adding edges to movements, add edges to these (indexed)
    // uber-turns.
    let mut uber_turn_entrances: MultiMap<DirectedRoadID, usize> = MultiMap::new();
    for (idx, ut) in uber_turns.iter().enumerate() {
        // Force the nodes to always match up in the graph for different vehicle types.
        nodes.get(Node::UberTurn(idx));

        // But actually, make sure this uber-turn only contains roads that can be used by this
        // vehicle.
        // TODO Need to test editing lanes inside an IntersectionCluster very carefully. See Mercer
        // and Dexter.
        if ut
            .path
            .iter()
            .all(|mvmnt| !mvmnt.to.lanes(constraints, map).is_empty())
        {
            uber_turn_entrances.insert(ut.entry(), idx);
        }
    }

    for r in map.all_roads() {
        for dr in r.id.both_directions() {
            let from = nodes.get(Node::Road(dr));
            if !dr.lanes(constraints, map).is_empty() {
                let indices = uber_turn_entrances.get(dr);
                if indices.is_empty() {
                    for mvmnt in map.get_movements_for(dr, constraints) {
                        input_graph.add_edge(
                            from,
                            nodes.get(Node::Road(mvmnt.to)),
                            round(
                                vehicle_cost(mvmnt.from, mvmnt, constraints, params, map)
                                    + zone_cost(mvmnt, constraints, map),
                            ),
                        );
                    }
                } else {
                    for idx in indices {
                        let ut = &uber_turns[*idx];

                        let mut sum_cost = Duration::ZERO;
                        for mvmnt in &ut.path {
                            sum_cost += vehicle_cost(mvmnt.from, *mvmnt, constraints, params, map)
                                + zone_cost(*mvmnt, constraints, map);
                        }
                        input_graph.add_edge(
                            from,
                            nodes.get(Node::UberTurn(*idx)),
                            round(sum_cost),
                        );
                        input_graph.add_edge(
                            nodes.get(Node::UberTurn(*idx)),
                            nodes.get(Node::Road(ut.exit())),
                            // The cost is already captured for entering the uber-turn
                            1,
                        );
                    }
                }
            }
        }
    }

    nodes.guarantee_node_ordering(&mut input_graph);
    input_graph.freeze();
    input_graph
}

/// This returns the pathfinding cost of crossing one road and turn. This is also expressed in
/// units of time. It factors in the ideal time to cross the space, along with penalties for
/// entering an access-restricted zone, taking an unprotected turn, and so on.
pub fn vehicle_cost(
    dr: DirectedRoadID,
    mvmnt: MovementID,
    constraints: PathConstraints,
    params: &RoutingParams,
    map: &Map,
) -> Duration {
    // TODO Creating the consolidated polyline sometimes fails. It's rare, so just workaround
    // temporarily by pretending the turn is 1m long.
    let (mvmnt_length, mvmnt_turn_type) = mvmnt
        .get(map)
        .map(|m| (m.geom.length(), m.turn_type))
        .unwrap_or((Distance::meters(1.0), TurnType::Straight));
    let max_speed = match constraints {
        PathConstraints::Car | PathConstraints::Bus | PathConstraints::Train => None,
        PathConstraints::Bike => Some(crate::MAX_BIKE_SPEED),
        PathConstraints::Pedestrian => unreachable!(),
    };
    let t1 = map.get_r(dr.id).center_pts.length()
        / Traversable::max_speed_along_road(dr, max_speed, constraints, map).0;
    let t2 =
        mvmnt_length / Traversable::max_speed_along_movement(mvmnt, max_speed, constraints, map);

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
    let rank_to = map.get_r(mvmnt.to.id).get_detailed_rank();
    if mvmnt_turn_type == unprotected_turn_type
        && rank_from < rank_to
        && map.get_i(mvmnt.parent).is_stop_sign()
    {
        base + params.unprotected_turn_penalty
    } else {
        base
    }
}

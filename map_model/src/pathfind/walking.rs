//! Pathfinding for pedestrians, as well as figuring out if somebody should use public transit.

use std::collections::HashMap;

use fast_paths::InputGraph;
use serde::{Deserialize, Serialize};

use geom::{Distance, Duration};

use crate::pathfind::engine::{CreateEngine, PathfindEngine};
use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::pathfind::vehicles::VehiclePathfinder;
use crate::pathfind::zone_cost;
use crate::pathfind::{round, unround};
use crate::{
    DirectedRoadID, IntersectionID, Map, PathConstraints, PathRequest, PathStep, PathStepV2,
    PathV2, Position, TransitRoute, TransitRouteID, TransitStopID, TurnType,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct SidewalkPathfinder {
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<WalkingNode>,
    use_transit: bool,
    engine: PathfindEngine,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub enum WalkingNode {
    /// false is src_i, true is dst_i
    SidewalkEndpoint(DirectedRoadID, bool),
    // TODO Lots of complexity below could be avoided by explicitly sticking TransitRouteID here too.
    // Worth it?
    RideTransit(TransitStopID),
    LeaveMap(IntersectionID),
}

impl WalkingNode {
    pub fn closest(pos: Position, map: &Map) -> WalkingNode {
        let lane = map.get_l(pos.lane());
        let dst_i = lane.length() - pos.dist_along() <= pos.dist_along();
        WalkingNode::SidewalkEndpoint(lane.get_directed_parent(), dst_i)
    }

    fn end_transit(pos: Position, map: &Map) -> WalkingNode {
        let l = map.get_l(pos.lane());
        if map.get_i(l.src_i).is_outgoing_border() && pos.dist_along() == Distance::ZERO {
            return WalkingNode::LeaveMap(l.src_i);
        }
        if map.get_i(l.dst_i).is_outgoing_border() && pos.dist_along() == l.length() {
            return WalkingNode::LeaveMap(l.dst_i);
        }
        WalkingNode::closest(pos, map)
    }
}

impl SidewalkPathfinder {
    pub fn empty() -> SidewalkPathfinder {
        SidewalkPathfinder {
            nodes: NodeMap::new(),
            use_transit: false,
            engine: PathfindEngine::Empty,
        }
    }

    pub fn new(
        map: &Map,
        use_transit: Option<(&VehiclePathfinder, &VehiclePathfinder)>,
        engine: &CreateEngine,
    ) -> SidewalkPathfinder {
        let mut nodes = NodeMap::new();
        for r in map.all_roads() {
            // Regardless of whether the road has sidewalks/shoulders on one or both sides, add
            // both. These could change later, and we want the node IDs to match up.
            for dr in r.id.both_directions() {
                for endpt in [true, false] {
                    nodes.get_or_insert(WalkingNode::SidewalkEndpoint(dr, endpt));
                }
            }
        }
        if use_transit.is_some() {
            // Add a node for each stop.
            for ts in map.all_transit_stops().keys() {
                nodes.get_or_insert(WalkingNode::RideTransit(*ts));
            }
            for i in map.all_outgoing_borders() {
                // We could filter for those with sidewalks, but eh
                nodes.get_or_insert(WalkingNode::LeaveMap(i.id));
            }
        }

        let input_graph = make_input_graph(&nodes, use_transit, map);
        let engine = engine.create(input_graph);

        SidewalkPathfinder {
            nodes,
            use_transit: use_transit.is_some(),
            engine,
        }
    }

    pub fn apply_edits(
        &mut self,
        map: &Map,
        use_transit: Option<(&VehiclePathfinder, &VehiclePathfinder)>,
    ) {
        if matches!(self.engine, PathfindEngine::Empty) {
            return;
        }

        let input_graph = make_input_graph(&self.nodes, use_transit, map);
        let engine = self.engine.reuse_ordering().create(input_graph);
        self.engine = engine;
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<PathV2> {
        if matches!(self.engine, PathfindEngine::Empty) {
            return None;
        }

        if req.start.lane() == req.end.lane() {
            return Some(one_step_walking_path(req, map));
        }
        let (raw_weight, raw_nodes) = self.engine.calculate_path(
            self.nodes.get(WalkingNode::closest(req.start, map)),
            self.nodes.get(WalkingNode::closest(req.end, map)),
        )?;
        let nodes: Vec<WalkingNode> = raw_nodes
            .into_iter()
            .map(|id| self.nodes.translate_id(id))
            .collect();
        let steps = walking_path_to_steps(nodes, map);
        let cost = unround(raw_weight);
        Some(PathV2::new(map, steps, req, cost, Vec::new()))
    }

    /// Attempt the pathfinding and see if we should ride public transit. If so, says (stop1,
    /// optional stop 2, route). If there's no stop 2, then ride transit off the border.
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(TransitStopID, Option<TransitStopID>, TransitRouteID)> {
        if matches!(self.engine, PathfindEngine::Empty) {
            return None;
        }

        assert!(self.use_transit);

        let (_, raw_nodes) = self.engine.calculate_path(
            self.nodes.get(WalkingNode::closest(start, map)),
            self.nodes.get(WalkingNode::end_transit(end, map)),
        )?;
        let nodes: Vec<WalkingNode> = raw_nodes
            .into_iter()
            .map(|id| self.nodes.translate_id(id))
            .collect();

        if false {
            println!("should_use_transit from {} to {}?", start, end);
            for n in &nodes {
                println!("- {:?}", n);
            }
        }

        let mut first_stop = None;
        let mut last_stop = None;
        let mut possible_routes: Vec<&TransitRoute> = Vec::new();
        for n in &nodes {
            match n {
                WalkingNode::RideTransit(stop2) => {
                    if let Some(stop1) = first_stop {
                        // Keep riding the same route?
                        // We need to do this check, because some transfers might be instantaneous
                        // at the same stop and involve no walking.
                        // Also need to make sure the stops are in the proper order. We might have
                        // a transfer, then try to hop on the first route again, but starting from
                        // a different point.
                        let mut filtered = possible_routes.clone();
                        filtered.retain(|r| {
                            let idx1 = r.stops.iter().position(|s| *s == stop1).unwrap();
                            let idx2 = r.stops.iter().position(|s| s == stop2);
                            idx2.map(|idx2| idx1 < idx2).unwrap_or(false)
                        });
                        if filtered.is_empty() {
                            // Aha, a transfer!
                            return Some((
                                first_stop.unwrap(),
                                // TODO I thought this should be impossible, but huge_seattle hits
                                // it. Workaround for now by just walking.
                                Some(last_stop?),
                                possible_routes[0].id,
                            ));
                        }
                        last_stop = Some(*stop2);
                        possible_routes = filtered;
                    } else {
                        first_stop = Some(*stop2);
                        possible_routes = map.get_routes_serving_stop(*stop2);
                        assert!(!possible_routes.is_empty());
                    }
                }
                WalkingNode::LeaveMap(i) => {
                    // Make sure the route actually leaves via the correct border!
                    if let Some(r) = possible_routes.iter().find(|r| {
                        r.end_border
                            .map(|l| map.get_l(l).dst_i == *i)
                            .unwrap_or(false)
                    }) {
                        return Some((first_stop.unwrap(), None, r.id));
                    }
                    // We can get close to the border, but should hop off at some stop.
                    return Some((
                        first_stop.unwrap(),
                        Some(last_stop.expect("impossible transit transfer")),
                        possible_routes[0].id,
                    ));
                }
                WalkingNode::SidewalkEndpoint(_, _) => {
                    if let Some(stop1) = first_stop {
                        return Some((
                            stop1,
                            Some(last_stop.expect("impossible transit transfer")),
                            possible_routes[0].id,
                        ));
                    }
                }
            }
        }
        None
    }

    pub fn all_costs_from(&self, start: Position, map: &Map) -> HashMap<DirectedRoadID, Duration> {
        if matches!(self.engine, PathfindEngine::Empty) {
            return HashMap::new();
        }

        let start = self.nodes.get(WalkingNode::closest(start, map));
        let raw_costs = if self.engine.is_dijkstra() {
            self.engine.all_costs_from(start)
        } else {
            // The CH engine doesn't support this!
            let input_graph = make_input_graph(&self.nodes, None, map);
            CreateEngine::Dijkstra
                .create(input_graph)
                .all_costs_from(start)
        };
        raw_costs
            .into_iter()
            .filter_map(|(k, v)| {
                // If we want to be more precise here, maybe take the min or max here of both
                // endpoints
                if let WalkingNode::SidewalkEndpoint(dr, _) = self.nodes.translate_id(k) {
                    Some((dr, unround(v)))
                } else {
                    None
                }
            })
            .collect()
    }
}

fn make_input_graph(
    nodes: &NodeMap<WalkingNode>,
    use_transit: Option<(&VehiclePathfinder, &VehiclePathfinder)>,
    map: &Map,
) -> InputGraph {
    let max_speed = Some(crate::MAX_WALKING_SPEED);
    let mut input_graph = InputGraph::new();

    for l in map.all_lanes() {
        if l.is_walkable() {
            // Sidewalks can be crossed in two directions. When there's a steep incline, of course
            // it flips.
            let n1 = nodes.get(WalkingNode::SidewalkEndpoint(
                l.get_directed_parent(),
                false,
            ));
            let n2 = nodes.get(WalkingNode::SidewalkEndpoint(l.get_directed_parent(), true));

            for (step, pair) in [
                (PathStep::Lane(l.id), (n1, n2)),
                (PathStep::ContraflowLane(l.id), (n2, n1)),
            ] {
                let mut cost =
                    l.length() / step.max_speed_along(max_speed, PathConstraints::Pedestrian, map);
                // TODO Tune this penalty, along with many others.
                if l.is_shoulder() {
                    cost = 2.0 * cost;
                }
                input_graph.add_edge(pair.0, pair.1, round(cost));
            }
        }
    }

    for t in map.all_turns() {
        if t.between_sidewalks() {
            let src = map.get_l(t.id.src);
            let dst = map.get_l(t.id.dst);
            let from = nodes.get(WalkingNode::SidewalkEndpoint(
                src.get_directed_parent(),
                src.dst_i == t.id.parent,
            ));
            let to = nodes.get(WalkingNode::SidewalkEndpoint(
                dst.get_directed_parent(),
                dst.dst_i == t.id.parent,
            ));

            let mut cost = t.geom.length()
                / PathStep::Turn(t.id).max_speed_along(max_speed, PathConstraints::Pedestrian, map)
                + zone_cost(t.id.to_movement(map), PathConstraints::Pedestrian, map);

            if t.turn_type == TurnType::UnmarkedCrossing {
                // TODO Add to RoutingParams
                cost = 3.0 * cost;
            }

            input_graph.add_edge(from, to, round(cost));
            input_graph.add_edge(to, from, round(cost));
        }
    }

    if let Some(graphs) = use_transit {
        transit_input_graph(&mut input_graph, nodes, map, graphs.0, graphs.1);
    }

    nodes.guarantee_node_ordering(&mut input_graph);
    input_graph.freeze();
    input_graph
}

fn transit_input_graph(
    input_graph: &mut InputGraph,
    nodes: &NodeMap<WalkingNode>,
    map: &Map,
    bus_graph: &VehiclePathfinder,
    train_graph: &VehiclePathfinder,
) {
    let max_speed = Some(crate::MAX_WALKING_SPEED);
    // Connect stops with both sidewalk endpoints, using the appropriate distance.
    for stop in map.all_transit_stops().values() {
        let ride_transit = nodes.get(WalkingNode::RideTransit(stop.id));
        let lane = map.get_l(stop.sidewalk_pos.lane());
        for (endpt, step) in [
            (false, PathStep::Lane(lane.id)),
            (true, PathStep::ContraflowLane(lane.id)),
        ] {
            let dist = if endpt {
                lane.length() - stop.sidewalk_pos.dist_along()
            } else {
                stop.sidewalk_pos.dist_along()
            };
            let cost = dist / step.max_speed_along(max_speed, PathConstraints::Pedestrian, map);
            // Add some extra penalty to using a stop. Otherwise a path might try to pass through
            // it uselessly.
            let penalty = Duration::seconds(10.0);
            let sidewalk = nodes.get(WalkingNode::SidewalkEndpoint(
                lane.get_directed_parent(),
                endpt,
            ));
            input_graph.add_edge(sidewalk, ride_transit, round(cost + penalty));
            input_graph.add_edge(ride_transit, sidewalk, round(cost + penalty));
        }
    }

    // Connect each adjacent stop along a route, with the cost based on how long it'll take a
    // transit vehicle to drive between the stops. Optimistically assume no waiting time at a stop.
    for route in map.all_transit_routes() {
        // TODO Also plug in border starts
        for pair in route.stops.windows(2) {
            let (stop1, stop2) = (map.get_ts(pair[0]), map.get_ts(pair[1]));
            let req = PathRequest::vehicle(stop1.driving_pos, stop2.driving_pos, route.route_type);
            let maybe_driving_cost = match route.route_type {
                PathConstraints::Bus => bus_graph.pathfind(req, map).map(|p| p.get_cost()),
                PathConstraints::Train => train_graph.pathfind(req, map).map(|p| p.get_cost()),
                _ => unreachable!(),
            };
            if let Some(driving_cost) = maybe_driving_cost {
                input_graph.add_edge(
                    nodes.get(WalkingNode::RideTransit(stop1.id)),
                    nodes.get(WalkingNode::RideTransit(stop2.id)),
                    round(driving_cost),
                );
            } else {
                panic!(
                    "No transit route from {} to {} now for {}! Prevent this edit",
                    stop1.driving_pos, stop2.driving_pos, route.long_name,
                );
            }
        }

        if let Some(l) = route.end_border {
            let stop1 = map.get_ts(*route.stops.last().unwrap());
            let req =
                PathRequest::vehicle(stop1.driving_pos, Position::end(l, map), route.route_type);
            let maybe_driving_cost = match route.route_type {
                PathConstraints::Bus => bus_graph.pathfind(req, map).map(|p| p.get_cost()),
                PathConstraints::Train => train_graph.pathfind(req, map).map(|p| p.get_cost()),
                _ => unreachable!(),
            };
            if let Some(driving_cost) = maybe_driving_cost {
                let border = map.get_i(map.get_l(l).dst_i);
                input_graph.add_edge(
                    nodes.get(WalkingNode::RideTransit(stop1.id)),
                    nodes.get(WalkingNode::LeaveMap(border.id)),
                    round(driving_cost),
                );
            } else {
                panic!(
                    "No transit route from {} to end of {} now for {}! Prevent this edit",
                    stop1.driving_pos, l, route.long_name,
                );
            }
        }
    }
}

// TODO Fold into reconstruct_path?
fn walking_path_to_steps(path: Vec<WalkingNode>, map: &Map) -> Vec<PathStepV2> {
    let mut steps = Vec::new();

    for pair in path.windows(2) {
        let (r1, r1_endpt) = match pair[0] {
            WalkingNode::SidewalkEndpoint(r, endpt) => (r, endpt),
            WalkingNode::RideTransit(_) => unreachable!(),
            WalkingNode::LeaveMap(_) => unreachable!(),
        };
        let r2 = match pair[1] {
            WalkingNode::SidewalkEndpoint(r, _) => r,
            WalkingNode::RideTransit(_) => unreachable!(),
            WalkingNode::LeaveMap(_) => unreachable!(),
        };

        if r1 == r2 {
            if r1_endpt {
                steps.push(PathStepV2::Contraflow(r1));
            } else {
                steps.push(PathStepV2::Along(r1));
            }
        } else {
            let i = if r1_endpt {
                r1.dst_i(map)
            } else {
                r1.src_i(map)
            };
            // Could assert the intersection matches (r2, r2_endpt).
            if let Some(t) =
                map.get_turn_between(r1.must_get_sidewalk(map), r2.must_get_sidewalk(map), i)
            {
                steps.push(PathStepV2::Movement(t.id.to_movement(map)));
            } else if let Some(t) =
                map.get_turn_between(r2.must_get_sidewalk(map), r1.must_get_sidewalk(map), i)
            {
                steps.push(PathStepV2::ContraflowMovement(t.id.to_movement(map)));
            } else {
                println!("walking_path_to_steps has a weird path:");
                for s in &path {
                    println!("- {:?}", s);
                }
                panic!(
                    "No turn from {} ({}) to {} ({}) at {}",
                    r1,
                    r1.must_get_sidewalk(map),
                    r2,
                    r2.must_get_sidewalk(map),
                    i
                );
            }
        }
    }

    // Don't start or end a path in a turn; sim layer breaks.
    if let PathStepV2::Movement(mvmnt) | PathStepV2::ContraflowMovement(mvmnt) = steps[0] {
        let lane = match steps[0] {
            PathStepV2::Movement(m) => m.from,
            PathStepV2::ContraflowMovement(m) => m.to,
            _ => unreachable!(),
        };
        if lane.src_i(map) == mvmnt.parent {
            steps.insert(0, PathStepV2::Contraflow(lane));
        } else {
            steps.insert(0, PathStepV2::Along(lane));
        }
    }
    if let PathStepV2::Movement(mvmnt) | PathStepV2::ContraflowMovement(mvmnt) =
        steps.last().cloned().unwrap()
    {
        let lane = match steps.last().unwrap() {
            PathStepV2::Movement(m) => m.to,
            PathStepV2::ContraflowMovement(m) => m.from,
            _ => unreachable!(),
        };
        if lane.src_i(map) == mvmnt.parent {
            steps.push(PathStepV2::Along(lane));
        } else {
            steps.push(PathStepV2::Contraflow(lane));
        }
    }

    steps
}

// TODO Do we even need this at all?
fn one_step_walking_path(req: PathRequest, map: &Map) -> PathV2 {
    let l = req.start.lane();
    // Weird case, but it can happen for walking from a building path to a stop that're actually at
    // the same spot.
    let (step_v2, step_v1) = if req.start.dist_along() <= req.end.dist_along() {
        (
            PathStepV2::Along(map.get_l(l).get_directed_parent()),
            PathStep::Lane(l),
        )
    } else {
        (
            PathStepV2::Contraflow(map.get_l(l).get_directed_parent()),
            PathStep::ContraflowLane(l),
        )
    };
    let mut cost = (req.start.dist_along() - req.end.dist_along()).abs()
        / step_v1.max_speed_along(
            Some(crate::MAX_WALKING_SPEED),
            PathConstraints::Pedestrian,
            map,
        );
    if map.get_l(l).is_shoulder() {
        cost = 2.0 * cost;
    }
    PathV2::new(map, vec![step_v2], req, cost, Vec::new())
}

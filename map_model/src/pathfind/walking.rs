//! Pathfinding for pedestrians using contraction hierarchies, as well as figuring out if somebody
//! should use public transit.

use std::cell::RefCell;
use std::collections::HashSet;

use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use serde::{Deserialize, Serialize};
use thread_local::ThreadLocal;

use geom::{Distance, Speed};

use crate::pathfind::driving::VehiclePathfinder;
use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{
    BusRoute, BusRouteID, BusStopID, IntersectionID, LaneID, Map, Path, PathConstraints,
    PathRequest, PathStep, Position,
};

#[derive(Serialize, Deserialize)]
pub struct SidewalkPathfinder {
    #[serde(serialize_with = "serialize_32", deserialize_with = "deserialize_32")]
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<WalkingNode>,
    use_transit: bool,

    #[serde(skip_serializing, skip_deserializing)]
    path_calc: ThreadLocal<RefCell<PathCalculator>>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub enum WalkingNode {
    /// false is src_i, true is dst_i
    SidewalkEndpoint(LaneID, bool),
    // TODO Lots of complexity below could be avoided by explicitly sticking BusRouteID here too.
    // Worth it?
    RideBus(BusStopID),
    LeaveMap(IntersectionID),
}

impl WalkingNode {
    pub fn closest(pos: Position, map: &Map) -> WalkingNode {
        let dst_i = map.get_l(pos.lane()).length() - pos.dist_along() <= pos.dist_along();
        WalkingNode::SidewalkEndpoint(pos.lane(), dst_i)
    }

    fn end_transit(pos: Position, map: &Map) -> WalkingNode {
        let l = map.get_l(pos.lane());
        if map.get_i(l.src_i).is_outgoing_border() {
            if pos.dist_along() == Distance::ZERO {
                return WalkingNode::LeaveMap(l.src_i);
            }
        }
        if map.get_i(l.dst_i).is_outgoing_border() {
            if pos.dist_along() == l.length() {
                return WalkingNode::LeaveMap(l.dst_i);
            }
        }
        WalkingNode::closest(pos, map)
    }
}

impl SidewalkPathfinder {
    pub fn new(
        map: &Map,
        use_transit: bool,
        bus_graph: &VehiclePathfinder,
        train_graph: &VehiclePathfinder,
    ) -> SidewalkPathfinder {
        let mut nodes = NodeMap::new();
        // We're assuming that to start with, no sidewalks are closed for construction!
        for l in map.all_lanes() {
            if l.is_walkable() {
                nodes.get_or_insert(WalkingNode::SidewalkEndpoint(l.id, true));
                nodes.get_or_insert(WalkingNode::SidewalkEndpoint(l.id, false));
            }
        }
        if use_transit {
            // Add a node for each bus stop.
            for bs in map.all_bus_stops().keys() {
                nodes.get_or_insert(WalkingNode::RideBus(*bs));
            }
            for i in map.all_outgoing_borders() {
                // We could filter for those with sidewalks, but eh
                nodes.get_or_insert(WalkingNode::LeaveMap(i.id));
            }
        }

        let graph = fast_paths::prepare(&make_input_graph(
            map,
            &nodes,
            use_transit,
            bus_graph,
            train_graph,
        ));
        SidewalkPathfinder {
            graph,
            nodes,
            use_transit,
            path_calc: ThreadLocal::new(),
        }
    }

    pub fn apply_edits(
        &mut self,
        map: &Map,
        bus_graph: &VehiclePathfinder,
        train_graph: &VehiclePathfinder,
    ) {
        // The NodeMap is all sidewalks, bus stops, and borders -- it won't change. So we can also
        // reuse the node ordering.
        let input_graph =
            make_input_graph(map, &self.nodes, self.use_transit, bus_graph, train_graph);
        let node_ordering = self.graph.get_node_ordering();
        self.graph = fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap();
    }

    /// Returns the raw nodes
    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<Vec<WalkingNode>> {
        assert_ne!(req.start.lane(), req.end.lane());
        let mut calc = self
            .path_calc
            .get_or(|| RefCell::new(fast_paths::create_calculator(&self.graph)))
            .borrow_mut();
        let raw_path = calc.calc_path(
            &self.graph,
            self.nodes.get(WalkingNode::closest(req.start, map)),
            self.nodes.get(WalkingNode::closest(req.end, map)),
        )?;
        Some(self.nodes.translate(&raw_path))
    }

    /// Attempt the pathfinding and see if we should ride a bus. If so, says (stop1, optional stop
    /// 2, route). If there's no stop 2, then ride the bus off the border.
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        let raw_path = fast_paths::calc_path(
            &self.graph,
            self.nodes.get(WalkingNode::closest(start, map)),
            self.nodes.get(WalkingNode::end_transit(end, map)),
        )?;

        let nodes = self.nodes.translate(&raw_path);
        if false {
            println!("should_use_transit from {} to {}?", start, end);
            for n in &nodes {
                println!("- {:?}", n);
            }
        }

        let mut first_stop = None;
        let mut last_stop = None;
        let mut possible_routes: Vec<&BusRoute> = Vec::new();
        for n in &nodes {
            match n {
                WalkingNode::RideBus(stop2) => {
                    if first_stop.is_none() {
                        first_stop = Some(*stop2);
                        possible_routes = map.get_routes_serving_stop(*stop2);
                        assert!(!possible_routes.is_empty());
                    } else {
                        // Keep riding the same route?
                        // We need to do this check, because some transfers might be instantaneous
                        // at the same stop and involve no walking.
                        // Also need to make sure the stops are in the proper order. We might have
                        // a transfer, then try to hop on the first route again, but starting from
                        // a different point.
                        let stop1 = first_stop.unwrap();
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
}

fn make_input_graph(
    map: &Map,
    nodes: &NodeMap<WalkingNode>,
    use_transit: bool,
    bus_graph: &VehiclePathfinder,
    train_graph: &VehiclePathfinder,
) -> InputGraph {
    let mut input_graph = InputGraph::new();

    for l in map.all_lanes() {
        if l.is_walkable()
            && map
                .get_r(l.parent)
                .access_restrictions
                .allow_through_traffic
                .contains(PathConstraints::Pedestrian)
        {
            let mut cost = walking_cost(l.length());
            // TODO Tune this penalty, along with many others.
            if l.is_shoulder() {
                cost *= 2;
            }
            let n1 = nodes.get(WalkingNode::SidewalkEndpoint(l.id, true));
            let n2 = nodes.get(WalkingNode::SidewalkEndpoint(l.id, false));
            input_graph.add_edge(n1, n2, cost);
            input_graph.add_edge(n2, n1, cost);
        }
    }

    for t in map.all_turns().values() {
        if t.between_sidewalks() {
            let from =
                WalkingNode::SidewalkEndpoint(t.id.src, map.get_l(t.id.src).dst_i == t.id.parent);
            let to =
                WalkingNode::SidewalkEndpoint(t.id.dst, map.get_l(t.id.dst).dst_i == t.id.parent);
            input_graph.add_edge(
                nodes.get(from),
                nodes.get(to),
                walking_cost(t.geom.length()),
            );
        }
    }

    if use_transit {
        transit_input_graph(&mut input_graph, map, nodes, bus_graph, train_graph);
    }

    input_graph.freeze();
    input_graph
}

fn transit_input_graph(
    input_graph: &mut InputGraph,
    map: &Map,
    nodes: &NodeMap<WalkingNode>,
    bus_graph: &VehiclePathfinder,
    train_graph: &VehiclePathfinder,
) {
    // Connect bus stops with both sidewalk endpoints, using the appropriate distance.
    for stop in map.all_bus_stops().values() {
        let ride_bus = nodes.get(WalkingNode::RideBus(stop.id));
        let lane = map.get_l(stop.sidewalk_pos.lane());
        for endpt in &[true, false] {
            let cost = if *endpt {
                walking_cost(lane.length() - stop.sidewalk_pos.dist_along())
            } else {
                walking_cost(stop.sidewalk_pos.dist_along())
            };
            // Add some extra penalty (equivalent to 1m) to using a bus stop. Otherwise a path
            // might try to pass through it uselessly.
            let penalty = 100;
            let sidewalk = nodes.get(WalkingNode::SidewalkEndpoint(lane.id, *endpt));
            input_graph.add_edge(sidewalk, ride_bus, cost + penalty);
            input_graph.add_edge(ride_bus, sidewalk, cost + penalty);
        }
    }

    let mut used_border_nodes = HashSet::new();

    // Connect each adjacent stop along a route, with the cost based on how long it'll take a
    // bus to drive between the stops. Optimistically assume no waiting time at a stop.
    for route in map.all_bus_routes() {
        // TODO Also plug in border starts
        for pair in route.stops.windows(2) {
            let (stop1, stop2) = (map.get_bs(pair[0]), map.get_bs(pair[1]));
            let graph = match route.route_type {
                PathConstraints::Bus => bus_graph,
                PathConstraints::Train => train_graph,
                _ => unreachable!(),
            };
            if let Some((_, driving_cost)) = graph.pathfind(
                &PathRequest {
                    start: stop1.driving_pos,
                    end: stop2.driving_pos,
                    constraints: route.route_type,
                },
                map,
            ) {
                input_graph.add_edge(
                    nodes.get(WalkingNode::RideBus(stop1.id)),
                    nodes.get(WalkingNode::RideBus(stop2.id)),
                    driving_cost,
                );
            } else {
                panic!(
                    "No bus route from {} to {} now for {}! Prevent this edit",
                    stop1.driving_pos, stop2.driving_pos, route.full_name,
                );
            }
        }

        if let Some(l) = route.end_border {
            let graph = match route.route_type {
                PathConstraints::Bus => bus_graph,
                PathConstraints::Train => train_graph,
                _ => unreachable!(),
            };
            let stop1 = map.get_bs(*route.stops.last().unwrap());
            if let Some((_, driving_cost)) = graph.pathfind(
                &PathRequest {
                    start: stop1.driving_pos,
                    end: Position::end(l, map),
                    constraints: route.route_type,
                },
                map,
            ) {
                let border = map.get_i(map.get_l(l).dst_i);
                input_graph.add_edge(
                    nodes.get(WalkingNode::RideBus(stop1.id)),
                    nodes.get(WalkingNode::LeaveMap(border.id)),
                    driving_cost,
                );
                used_border_nodes.insert(border.id);
            } else {
                panic!(
                    "No bus route from {} to end of {} now for {}! Prevent this edit",
                    stop1.driving_pos, l, route.full_name,
                );
            }
        }
    }

    // InputGraph strips out nodes with no edges, so verify the last border is connected to
    // something.
    // Since no paths will ever reach this unused node, this won't affect results.
    // TODO Upstream a method in InputGraph to do this more clearly.
    if let Some(i) = map.all_outgoing_borders().last() {
        if !used_border_nodes.contains(&i.id) {
            let some_sidewalk = map
                .all_lanes()
                .into_iter()
                .find(|l| l.is_walkable())
                .expect("no sidewalks in map");
            input_graph.add_edge(
                nodes.get(WalkingNode::LeaveMap(i.id)),
                nodes.get(WalkingNode::SidewalkEndpoint(some_sidewalk.id, true)),
                1,
            );
        }
    }
}

/// The cost is time in seconds, rounded to a usize
pub fn walking_cost(dist: Distance) -> usize {
    let walking_speed = Speed::meters_per_second(1.34);
    let time = dist / walking_speed;
    (time.inner_seconds().round() as usize).max(1)
}

pub fn walking_path_to_steps(path: Vec<WalkingNode>, map: &Map) -> Vec<PathStep> {
    let mut steps: Vec<PathStep> = Vec::new();

    for pair in path.windows(2) {
        let (l1, l1_endpt) = match pair[0] {
            WalkingNode::SidewalkEndpoint(l, endpt) => (l, endpt),
            WalkingNode::RideBus(_) => unreachable!(),
            WalkingNode::LeaveMap(_) => unreachable!(),
        };
        let l2 = match pair[1] {
            WalkingNode::SidewalkEndpoint(l, _) => l,
            WalkingNode::RideBus(_) => unreachable!(),
            WalkingNode::LeaveMap(_) => unreachable!(),
        };

        if l1 == l2 {
            if l1_endpt {
                steps.push(PathStep::ContraflowLane(l1));
            } else {
                steps.push(PathStep::Lane(l1));
            }
        } else {
            let i = {
                let l = map.get_l(l1);
                if l1_endpt {
                    l.dst_i
                } else {
                    l.src_i
                }
            };
            // Could assert the intersection matches (l2, l2_endpt).
            if let Some(turn) = map.get_turn_between(l1, l2, i) {
                steps.push(PathStep::Turn(turn));
            } else {
                println!("walking_path_to_steps has a weird path:");
                for s in &path {
                    println!("- {:?}", s);
                }
                panic!("No turn from {} to {} at {}", l1, l2, i);
            }
        }
    }

    // Don't start or end a path in a turn; sim layer breaks.
    if let PathStep::Turn(t) = steps[0] {
        let lane = map.get_l(t.src);
        if lane.src_i == t.parent {
            steps.insert(0, PathStep::ContraflowLane(lane.id));
        } else {
            steps.insert(0, PathStep::Lane(lane.id));
        }
    }
    if let PathStep::Turn(t) = steps.last().unwrap() {
        let lane = map.get_l(t.dst);
        if lane.src_i == t.parent {
            steps.push(PathStep::Lane(lane.id));
        } else {
            steps.push(PathStep::ContraflowLane(lane.id));
        }
    }

    steps
}

pub fn one_step_walking_path(req: &PathRequest, map: &Map) -> Path {
    // Weird case, but it can happen for walking from a building path to a bus stop that're
    // actually at the same spot.
    if req.start.dist_along() == req.end.dist_along() {
        Path::new(
            map,
            vec![PathStep::Lane(req.start.lane())],
            req.clone(),
            Vec::new(),
        )
    } else if req.start.dist_along() < req.end.dist_along() {
        Path::new(
            map,
            vec![PathStep::Lane(req.start.lane())],
            req.clone(),
            Vec::new(),
        )
    } else {
        Path::new(
            map,
            vec![PathStep::ContraflowLane(req.start.lane())],
            req.clone(),
            Vec::new(),
        )
    }
}

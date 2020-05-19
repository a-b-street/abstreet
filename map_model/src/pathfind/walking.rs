use crate::pathfind::driving::VehiclePathfinder;
use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{
    BusRouteID, BusStopID, LaneID, Map, Path, PathConstraints, PathRequest, PathStep, Position,
};
use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use geom::{Distance, Speed};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use thread_local::ThreadLocal;

#[derive(Serialize, Deserialize)]
pub struct SidewalkPathfinder {
    #[serde(serialize_with = "serialize_32", deserialize_with = "deserialize_32")]
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<Node>,
    use_transit: bool,

    #[serde(skip_serializing, skip_deserializing)]
    path_calc: ThreadLocal<RefCell<PathCalculator>>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
enum Node {
    // false is src_i, true is dst_i
    SidewalkEndpoint(LaneID, bool),
    RideBus(BusStopID),
}

impl SidewalkPathfinder {
    pub fn new(map: &Map, use_transit: bool, bus_graph: &VehiclePathfinder) -> SidewalkPathfinder {
        let mut nodes = NodeMap::new();
        // We're assuming that to start with, no sidewalks are closed for construction!
        for l in map.all_lanes() {
            if l.is_sidewalk() {
                nodes.get_or_insert(Node::SidewalkEndpoint(l.id, true));
                nodes.get_or_insert(Node::SidewalkEndpoint(l.id, false));
            }
        }
        if use_transit {
            // Add a node for each bus stop.
            for stop in map.all_bus_stops().values() {
                nodes.get_or_insert(Node::RideBus(stop.id));
            }
        }

        let graph = fast_paths::prepare(&make_input_graph(map, &nodes, use_transit, bus_graph));
        SidewalkPathfinder {
            graph,
            nodes,
            use_transit,
            path_calc: ThreadLocal::new(),
        }
    }

    pub fn apply_edits(&mut self, map: &Map, bus_graph: &VehiclePathfinder) {
        // The NodeMap is all sidewalks and bus stops -- it won't change. So we can also reuse the
        // node ordering.
        let input_graph = make_input_graph(map, &self.nodes, self.use_transit, bus_graph);
        let node_ordering = self.graph.get_node_ordering();
        self.graph = fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap();
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<Path> {
        // Special-case one-step paths.
        // TODO Maybe we don't need these special cases anymore.
        if req.start.lane() == req.end.lane() {
            // Weird case, but it can happen for walking from a building path to a bus stop that're
            // actually at the same spot.
            if req.start.dist_along() == req.end.dist_along() {
                return Some(Path::new(
                    map,
                    vec![PathStep::Lane(req.start.lane())],
                    req.start.dist_along(),
                ));
            } else if req.start.dist_along() < req.end.dist_along() {
                return Some(Path::new(
                    map,
                    vec![PathStep::Lane(req.start.lane())],
                    req.end.dist_along(),
                ));
            } else {
                return Some(Path::new(
                    map,
                    vec![PathStep::ContraflowLane(req.start.lane())],
                    req.end.dist_along(),
                ));
            }
        }

        let mut calc = self
            .path_calc
            .get_or(|| RefCell::new(fast_paths::create_calculator(&self.graph)))
            .borrow_mut();
        let raw_path = calc.calc_path(
            &self.graph,
            self.nodes.get(closest_node(req.start, map)),
            self.nodes.get(closest_node(req.end, map)),
        )?;
        let path = self.nodes.translate(&raw_path);

        let mut steps: Vec<PathStep> = Vec::new();

        for pair in path.windows(2) {
            let (l1, l1_endpt) = match pair[0] {
                Node::SidewalkEndpoint(l, endpt) => (l, endpt),
                Node::RideBus(_) => unreachable!(),
            };
            let l2 = match pair[1] {
                Node::SidewalkEndpoint(l, _) => l,
                Node::RideBus(_) => unreachable!(),
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
                let turn = map.get_turn_between(l1, l2, i).unwrap();
                steps.push(PathStep::Turn(turn));
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

        Some(Path::new(map, steps, req.end.dist_along()))
    }

    // Attempt the pathfinding and see if we should ride a bus.
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        let raw_path = fast_paths::calc_path(
            &self.graph,
            self.nodes.get(closest_node(start, map)),
            self.nodes.get(closest_node(end, map)),
        )?;

        let mut nodes = self.nodes.translate(&raw_path);
        let mut first_stop = None;
        for n in &nodes {
            if let Node::RideBus(stop) = n {
                first_stop = Some(*stop);
                break;
            }
        }
        let first_stop = first_stop?;
        let possible_routes = map.get_routes_serving_stop(first_stop);

        nodes.reverse();
        for n in nodes {
            if let Node::RideBus(stop2) = n {
                if let Some(route) = possible_routes.iter().find(|r| r.stops.contains(&stop2)) {
                    assert_ne!(first_stop, stop2);
                    return Some((first_stop, stop2, route.id));
                }
            }
        }
        unreachable!()
    }
}

fn closest_node(pos: Position, map: &Map) -> Node {
    let dst_i = map.get_l(pos.lane()).length() - pos.dist_along() <= pos.dist_along();
    Node::SidewalkEndpoint(pos.lane(), dst_i)
}

fn make_input_graph(
    map: &Map,
    nodes: &NodeMap<Node>,
    use_transit: bool,
    bus_graph: &VehiclePathfinder,
) -> InputGraph {
    let mut input_graph = InputGraph::new();

    for l in map.all_lanes() {
        if l.is_sidewalk() {
            let cost = to_s(l.length());
            let n1 = nodes.get(Node::SidewalkEndpoint(l.id, true));
            let n2 = nodes.get(Node::SidewalkEndpoint(l.id, false));
            input_graph.add_edge(n1, n2, cost);
            input_graph.add_edge(n2, n1, cost);
        }
    }

    for t in map.all_turns().values() {
        if t.between_sidewalks() {
            let from = Node::SidewalkEndpoint(t.id.src, map.get_l(t.id.src).dst_i == t.id.parent);
            let to = Node::SidewalkEndpoint(t.id.dst, map.get_l(t.id.dst).dst_i == t.id.parent);
            input_graph.add_edge(nodes.get(from), nodes.get(to), to_s(t.geom.length()));
        }
    }

    if use_transit {
        // Connect bus stops with both sidewalk endpoints, using the appropriate distance.
        for stop in map.all_bus_stops().values() {
            let ride_bus = nodes.get(Node::RideBus(stop.id));
            let lane = map.get_l(stop.sidewalk_pos.lane());
            for endpt in &[true, false] {
                let cost = if *endpt {
                    to_s(lane.length() - stop.sidewalk_pos.dist_along())
                } else {
                    to_s(stop.sidewalk_pos.dist_along())
                };
                // Add some extra penalty (equivalent to 1m) to using a bus stop. Otherwise a path
                // might try to pass through it uselessly.
                let penalty = 100;
                let sidewalk = nodes.get(Node::SidewalkEndpoint(lane.id, *endpt));
                input_graph.add_edge(sidewalk, ride_bus, cost + penalty);
                input_graph.add_edge(ride_bus, sidewalk, cost + penalty);
            }
        }

        // Connect each adjacent stop along a route, with the cost based on how long it'll take a
        // bus to drive between the stops. Optimistically assume no waiting time at a stop.
        for route in map.get_all_bus_routes() {
            for (stop1, stop2) in
                route
                    .stops
                    .iter()
                    .zip(route.stops.iter().skip(1))
                    .chain(std::iter::once((
                        route.stops.last().unwrap(),
                        &route.stops[0],
                    )))
            {
                if let Some((_, driving_cost)) = bus_graph.pathfind(
                    &PathRequest {
                        start: map.get_bs(*stop1).driving_pos,
                        end: map.get_bs(*stop2).driving_pos,
                        constraints: PathConstraints::Bus,
                    },
                    map,
                ) {
                    input_graph.add_edge(
                        nodes.get(Node::RideBus(*stop1)),
                        nodes.get(Node::RideBus(*stop2)),
                        driving_cost,
                    );
                } else {
                    panic!(
                        "No bus route from {} to {} now! Prevent this edit",
                        map.get_bs(*stop1).driving_pos,
                        map.get_bs(*stop2).driving_pos
                    );
                }
            }
        }
    }
    input_graph.freeze();
    input_graph
}

fn to_s(dist: Distance) -> usize {
    let walking_speed = Speed::meters_per_second(1.34);
    let time = dist / walking_speed;
    (time.inner_seconds().round() as usize).max(1)
}

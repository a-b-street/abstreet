use crate::pathfind::driving::VehiclePathfinder;
use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{
    BusRouteID, BusStopID, IntersectionID, LaneID, Map, Path, PathConstraints, PathRequest,
    PathStep, Position,
};
use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use geom::{Distance, Speed};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashSet;
use thread_local::ThreadLocal;

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
    // false is src_i, true is dst_i
    SidewalkEndpoint(LaneID, bool),
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
            if l.is_sidewalk() {
                nodes.get_or_insert(WalkingNode::SidewalkEndpoint(l.id, true));
                nodes.get_or_insert(WalkingNode::SidewalkEndpoint(l.id, false));
            }
        }
        if use_transit {
            // Add a node for each bus stop.
            for stop in map.all_bus_stops().values() {
                nodes.get_or_insert(WalkingNode::RideBus(stop.id));
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

    // Returns the raw nodes
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

    // Attempt the pathfinding and see if we should ride a bus. If so, says (stop1, optional stop
    // 2, route). If there's no stop 2, then ride the bus off the border.
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

        let mut nodes = self.nodes.translate(&raw_path);
        let mut first_stop = None;
        for n in &nodes {
            if let WalkingNode::RideBus(stop) = n {
                first_stop = Some(*stop);
                break;
            }
        }
        let first_stop = first_stop?;
        let possible_routes = map.get_routes_serving_stop(first_stop);

        nodes.reverse();
        // Should we ride that first bus off-map?
        if let (WalkingNode::LeaveMap(_), WalkingNode::RideBus(stop2)) = (nodes[0], nodes[1]) {
            if let Some(route) = possible_routes.iter().find(|r| r.stops.contains(&stop2)) {
                // The only way there'd only be one RideBus node in the path is this case. If
                // there's a transfer, there are at least two stops.
                return Some((first_stop, None, route.id));
            }
        }

        for n in nodes {
            if let WalkingNode::RideBus(stop2) = n {
                // If there's no route, this actually implies a transfer! TODO Handle that.
                if let Some(route) = possible_routes.iter().find(|r| r.stops.contains(&stop2)) {
                    assert_ne!(first_stop, stop2);
                    return Some((first_stop, Some(stop2), route.id));
                }
            }
        }
        unreachable!()
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
        if l.is_sidewalk()
            && map
                .get_r(l.parent)
                .allow_through_traffic
                .contains(PathConstraints::Pedestrian)
        {
            let cost = walking_cost(l.length());
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
            // TODO Various bugs still.
            if true {
                continue;
            }
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
                // There are potentially two SidewalkEndpoints where someone might try to
                // vanish
                if let Some(l) = border
                    .get_incoming_lanes(map, PathConstraints::Pedestrian)
                    .next()
                {
                    used_border_nodes.insert(border.id);
                    input_graph.add_edge(
                        nodes.get(WalkingNode::LeaveMap(border.id)),
                        nodes.get(WalkingNode::SidewalkEndpoint(l, true)),
                        driving_cost,
                    );
                }
                if let Some(l) = border
                    .get_outgoing_lanes(map, PathConstraints::Pedestrian)
                    .get(0)
                {
                    used_border_nodes.insert(border.id);
                    input_graph.add_edge(
                        nodes.get(WalkingNode::LeaveMap(border.id)),
                        nodes.get(WalkingNode::SidewalkEndpoint(*l, false)),
                        driving_cost,
                    );
                }
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
                .find(|l| l.is_sidewalk())
                .expect("no sidewalks in map");
            input_graph.add_edge(
                nodes.get(WalkingNode::LeaveMap(i.id)),
                nodes.get(WalkingNode::SidewalkEndpoint(some_sidewalk.id, true)),
                1,
            );
        }
    }
}

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
            req.start.dist_along(),
            Vec::new(),
        )
    } else if req.start.dist_along() < req.end.dist_along() {
        Path::new(
            map,
            vec![PathStep::Lane(req.start.lane())],
            req.end.dist_along(),
            Vec::new(),
        )
    } else {
        Path::new(
            map,
            vec![PathStep::ContraflowLane(req.start.lane())],
            req.end.dist_along(),
            Vec::new(),
        )
    }
}

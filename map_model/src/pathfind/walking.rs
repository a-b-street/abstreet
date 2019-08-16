use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{
    BusRouteID, BusStopID, DirectedRoadID, IntersectionID, LaneID, LaneType, Map, Path,
    PathRequest, PathStep, Position,
};
use fast_paths::{FastGraph, InputGraph, PathCalculator};
use geom::Distance;
use serde_derive::{Deserialize, Serialize};
use std::cell::RefCell;
use thread_local::ThreadLocal;

#[derive(Serialize, Deserialize)]
pub struct SidewalkPathfinder {
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<Node>,

    #[serde(skip_serializing, skip_deserializing)]
    path_calc: ThreadLocal<RefCell<PathCalculator>>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum Node {
    // Direction determined later
    Cross(DirectedRoadID),
    RideBus(BusStopID),
}

impl SidewalkPathfinder {
    pub fn new(map: &Map, use_transit: bool) -> SidewalkPathfinder {
        let mut input_graph = InputGraph::new();
        let mut nodes = NodeMap::new();

        for t in map.all_turns().values() {
            if !t.between_sidewalks() || !map.is_turn_allowed(t.id) {
                continue;
            }
            // Duplicate edges in InputGraph will be removed.
            let length = map.get_l(t.id.src).length() + t.geom.length();
            let length_cm = (length.inner_meters() * 100.0).round() as usize;

            input_graph.add_edge(
                nodes.get_or_insert(lane_to_node(t.id.src, map)),
                nodes.get_or_insert(lane_to_node(t.id.dst, map)),
                length_cm,
            );
        }

        if use_transit {
            // Add a node for each bus stop, and a "free" cost of 1 (fast_paths ignores 0-weight
            // edges) for moving between the stop and sidewalk.
            for stop in map.all_bus_stops().values() {
                let cross_lane = nodes.get(lane_to_node(stop.sidewalk_pos.lane(), map));
                let ride_bus = nodes.get_or_insert(Node::RideBus(stop.id));
                input_graph.add_edge(cross_lane, ride_bus, 1);
                input_graph.add_edge(ride_bus, cross_lane, 1);
            }

            // Connect each adjacent stop along a route, again with a "free" cost.
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
                    input_graph.add_edge(
                        nodes.get(Node::RideBus(*stop1)),
                        nodes.get(Node::RideBus(*stop2)),
                        1,
                    );
                }
            }
        }
        input_graph.freeze();
        let graph = fast_paths::prepare(&input_graph);

        SidewalkPathfinder {
            graph,
            nodes,
            path_calc: ThreadLocal::new(),
        }
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<Path> {
        // Special-case one-step paths.
        if req.start.lane() == req.end.lane() {
            let len = map.get_l(req.start.lane()).length();
            // Weird case, but it can happen for walking from a building path to a bus stop that're
            // actually at the same spot.
            if req.start.dist_along() == req.end.dist_along() {
                return Some(Path::new(
                    map,
                    vec![PathStep::Lane(req.start.lane())],
                    req.start.dist_along(),
                    len,
                ));
            } else if req.start.dist_along() < req.end.dist_along() {
                return Some(Path::new(
                    map,
                    vec![PathStep::Lane(req.start.lane())],
                    req.end.dist_along(),
                    len,
                ));
            } else {
                return Some(Path::new(
                    map,
                    vec![PathStep::ContraflowLane(req.start.lane())],
                    req.end.dist_along(),
                    len,
                ));
            }
        }

        let mut calc = self
            .path_calc
            .get_or(|| Box::new(RefCell::new(fast_paths::create_calculator(&self.graph))))
            .borrow_mut();
        let raw_path = calc.calc_path(
            &self.graph,
            self.nodes.maybe_get(lane_to_node(req.start.lane(), map))?,
            self.nodes.maybe_get(lane_to_node(req.end.lane(), map))?,
        )?;
        let path = self.nodes.translate(&raw_path);

        let mut steps: Vec<PathStep> = Vec::new();
        // If the request starts at the beginning/end of a lane, still include that as the first
        // PathStep. Sim layer breaks otherwise.
        let mut current_i: Option<IntersectionID> = None;

        for pair in path.windows(2) {
            let lane1 = match pair[0] {
                Node::Cross(dr) => map.get_l(get_sidewalk(dr, map)),
                Node::RideBus(_) => unreachable!(),
            };
            let l2 = match pair[1] {
                Node::Cross(dr) => get_sidewalk(dr, map),
                Node::RideBus(_) => unreachable!(),
            };

            let fwd_t = map.get_turn_between(lane1.id, l2, lane1.dst_i);
            let back_t = map.get_turn_between(lane1.id, l2, lane1.src_i);
            // TODO If both are available, we sort of need to lookahead to pick the better one.
            // Oh well.
            if fwd_t.is_some() {
                if current_i != Some(lane1.dst_i) {
                    steps.push(PathStep::Lane(lane1.id));
                }
                steps.push(PathStep::Turn(fwd_t.unwrap()));
                current_i = Some(lane1.dst_i);
            } else {
                if current_i != Some(lane1.src_i) {
                    steps.push(PathStep::ContraflowLane(lane1.id));
                }
                steps.push(PathStep::Turn(back_t.unwrap()));
                current_i = Some(lane1.src_i);
            }
        }

        // Don't end a path in a turn; sim layer breaks.
        let last_lane = match path.last().unwrap() {
            Node::Cross(dr) => map.get_l(get_sidewalk(*dr, map)),
            Node::RideBus(_) => unreachable!(),
        };
        if Some(last_lane.src_i) == current_i {
            steps.push(PathStep::Lane(last_lane.id));
        } else if Some(last_lane.dst_i) == current_i {
            steps.push(PathStep::ContraflowLane(last_lane.id));
        } else {
            unreachable!();
        }

        Some(Path::new(
            map,
            steps,
            req.end.dist_along(),
            Distance::centimeters(raw_path.get_weight()),
        ))
    }

    // Attempt the pathfinding and see if we should ride a bus.
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        // TODO maybe_get is a temporaryish hack -- some sidewalks are actually totally
        // disconnected, so there's no node for them. Just fail the pathfinding. Really this is a
        // bug in turn creation though.
        let raw_path = fast_paths::calc_path(
            &self.graph,
            self.nodes.maybe_get(lane_to_node(start.lane(), map))?,
            self.nodes.maybe_get(lane_to_node(end.lane(), map))?,
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
                    return Some((first_stop, stop2, route.id));
                }
            }
        }
        unreachable!()
    }
}

fn lane_to_node(l: LaneID, map: &Map) -> Node {
    Node::Cross(map.get_l(l).get_directed_parent(map))
}

fn get_sidewalk(dr: DirectedRoadID, map: &Map) -> LaneID {
    let r = map.get_r(dr.id);
    let lanes = if dr.forwards {
        &r.children_forwards
    } else {
        &r.children_backwards
    };
    for (id, lt) in lanes {
        if *lt == LaneType::Sidewalk {
            return *id;
        }
    }
    panic!("{} has no sidewalk", dr);
}

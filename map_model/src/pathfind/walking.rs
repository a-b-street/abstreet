use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::{
    BusRouteID, BusStopID, DirectedRoadID, IntersectionID, LaneID, LaneType, Map, Path,
    PathRequest, PathStep, Position,
};
use fast_paths::{FastGraph, InputGraph};
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SidewalkPathfinder {
    graph: FastGraph,
    #[serde(deserialize_with = "deserialize_nodemap")]
    nodes: NodeMap<Node>,
}

// TODO Possibly better representation is just a BusStopID as a node, and a separate map for
// (BusStopID, BusStopID) to BusRouteID.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum Node {
    // Direction determined later
    Cross(DirectedRoadID),
    RideBus(BusStopID, BusStopID, BusRouteID),
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

        // Add nodes for all the bus rides. No transfers.
        if use_transit {
            for stop1 in map.all_bus_stops().values() {
                let src_l = nodes.get_or_insert(lane_to_node(stop1.sidewalk_pos.lane(), map));
                for (stop2, route) in map.get_connected_bus_stops(stop1.id).into_iter() {
                    let dst_l = nodes
                        .get_or_insert(lane_to_node(map.get_bs(stop2).sidewalk_pos.lane(), map));
                    let ride_bus = nodes.get_or_insert(Node::RideBus(stop1.id, stop2, route));

                    // TODO Hardcode a nice, cheap cost of 1 (fast_paths ignores 0-weight edges)
                    // for riding the bus.
                    input_graph.add_edge(src_l, ride_bus, 1);
                    input_graph.add_edge(ride_bus, dst_l, 1);
                }
            }
        }
        input_graph.freeze();
        let graph = fast_paths::prepare(&input_graph);

        SidewalkPathfinder { graph, nodes }
    }

    pub fn pathfind(&self, req: &PathRequest, map: &Map) -> Option<Path> {
        // Special-case one-step paths.
        if req.start.lane() == req.end.lane() {
            assert!(req.start.dist_along() != req.end.dist_along());
            if req.start.dist_along() < req.end.dist_along() {
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

        let raw_path = fast_paths::calc_path(
            &self.graph,
            self.nodes.get(lane_to_node(req.start.lane(), map)),
            self.nodes.get(lane_to_node(req.end.lane(), map)),
        )?;
        let path = self.nodes.translate(&raw_path);

        let mut steps: Vec<PathStep> = Vec::new();
        // If the request starts at the beginning/end of a lane, still include that as the first
        // PathStep. Sim layer breaks otherwise.
        let mut current_i: Option<IntersectionID> = None;

        for pair in path.windows(2) {
            let lane1 = match pair[0] {
                Node::Cross(dr) => map.get_l(get_sidewalk(dr, map)),
                Node::RideBus(_, _, _) => unreachable!(),
            };
            let l2 = match pair[1] {
                Node::Cross(dr) => get_sidewalk(dr, map),
                Node::RideBus(_, _, _) => unreachable!(),
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
            Node::RideBus(_, _, _) => unreachable!(),
        };
        if Some(last_lane.src_i) == current_i {
            steps.push(PathStep::Lane(last_lane.id));
        } else if Some(last_lane.dst_i) == current_i {
            steps.push(PathStep::ContraflowLane(last_lane.id));
        } else {
            unreachable!();
        }

        Some(Path::new(map, steps, req.end.dist_along()))
    }

    // Attempt the pathfinding and see if riding a bus is a step.
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, BusStopID, BusRouteID)> {
        let raw_path = fast_paths::calc_path(
            &self.graph,
            self.nodes.get(lane_to_node(start.lane(), map)),
            self.nodes.get(lane_to_node(end.lane(), map)),
        )?;

        for node in self.nodes.translate(&raw_path) {
            if let Node::RideBus(stop1, stop2, route) = node {
                return Some((stop1, stop2, route));
            }
        }
        None
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

//! Pathfinding for pedestrians using contraction hierarchies, as well as figuring out if somebody
//! should use public transit.

use std::cell::RefCell;

use fast_paths::{deserialize_32, serialize_32, FastGraph, InputGraph, PathCalculator};
use serde::{Deserialize, Serialize};
use thread_local::ThreadLocal;

use geom::{Distance, Duration};

use crate::pathfind::ch::{round, unround};
use crate::pathfind::dijkstra;
use crate::pathfind::node_map::{deserialize_nodemap, NodeMap};
use crate::pathfind::vehicles::VehiclePathfinder;
use crate::pathfind::zone_cost;
use crate::{
    BusRoute, BusRouteID, BusStopID, DirectedRoadID, IntersectionID, Map, MovementID,
    PathConstraints, PathRequest, PathStep, PathStepV2, PathV2, Position,
};

#[derive(Serialize, Deserialize)]
pub struct SidewalkPathfinder {
    translator: SidewalkPathTranslator,
    #[serde(serialize_with = "serialize_32", deserialize_with = "deserialize_32")]
    graph: FastGraph,

    #[serde(skip_serializing, skip_deserializing)]
    path_calc: ThreadLocal<RefCell<PathCalculator>>,
}

/// Used for both contraction hierarchies and Dijkstra's
#[derive(Serialize, Deserialize)]
pub struct SidewalkPathTranslator {
    #[serde(deserialize_with = "deserialize_nodemap")]
    pub nodes: NodeMap<WalkingNode>,
    use_transit: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash, Serialize, Deserialize)]
pub enum WalkingNode {
    /// false is src_i, true is dst_i
    SidewalkEndpoint(DirectedRoadID, bool),
    // TODO Lots of complexity below could be avoided by explicitly sticking BusRouteID here too.
    // Worth it?
    RideBus(BusStopID),
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

impl SidewalkPathTranslator {
    pub fn just_walking(map: &Map) -> SidewalkPathTranslator {
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

        SidewalkPathTranslator {
            nodes,
            use_transit: false,
        }
    }

    pub fn walking_with_transit(map: &Map) -> SidewalkPathTranslator {
        let mut translator = SidewalkPathTranslator::just_walking(map);
        // Add a node for each bus stop.
        for bs in map.all_bus_stops().keys() {
            translator.nodes.get_or_insert(WalkingNode::RideBus(*bs));
        }
        for i in map.all_outgoing_borders() {
            // We could filter for those with sidewalks, but eh
            translator.nodes.get_or_insert(WalkingNode::LeaveMap(i.id));
        }
        translator
    }

    pub fn make_input_graph(&self, map: &Map, bus_graph: Option<&VehiclePathfinder>) -> InputGraph {
        let max_speed = Some(crate::MAX_WALKING_SPEED);
        let mut input_graph = InputGraph::new();

        for l in map.all_lanes().values() {
            if l.is_walkable() {
                // Sidewalks can be crossed in two directions. When there's a steep incline, of course
                // it flips.
                let n1 = self.nodes.get(WalkingNode::SidewalkEndpoint(
                    l.get_directed_parent(),
                    false,
                ));
                let n2 = self
                    .nodes
                    .get(WalkingNode::SidewalkEndpoint(l.get_directed_parent(), true));

                for (step, pair) in [
                    (PathStep::Lane(l.id), (n1, n2)),
                    (PathStep::ContraflowLane(l.id), (n2, n1)),
                ] {
                    let mut cost = l.length()
                        / step.max_speed_along(max_speed, PathConstraints::Pedestrian, map);
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
                let from = WalkingNode::SidewalkEndpoint(
                    src.get_directed_parent(),
                    src.dst_i == t.id.parent,
                );
                let to = WalkingNode::SidewalkEndpoint(
                    dst.get_directed_parent(),
                    dst.dst_i == t.id.parent,
                );
                let cost = t.geom.length()
                    / PathStep::Turn(t.id).max_speed_along(
                        max_speed,
                        PathConstraints::Pedestrian,
                        map,
                    );
                input_graph.add_edge(
                    self.nodes.get(from),
                    self.nodes.get(to),
                    round(
                        cost + zone_cost(t.id.to_movement(map), PathConstraints::Pedestrian, map),
                    ),
                );
            }
        }

        if self.use_transit {
            self.transit_input_graph(&mut input_graph, map, bus_graph.unwrap());
        }

        self.nodes.guarantee_node_ordering(&mut input_graph);
        input_graph.freeze();
        input_graph
    }

    fn transit_input_graph(
        &self,
        input_graph: &mut InputGraph,
        map: &Map,
        bus_graph: &VehiclePathfinder,
    ) {
        let max_speed = Some(crate::MAX_WALKING_SPEED);
        // Connect bus stops with both sidewalk endpoints, using the appropriate distance.
        for stop in map.all_bus_stops().values() {
            let ride_bus = self.nodes.get(WalkingNode::RideBus(stop.id));
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
                // Add some extra penalty to using a bus stop. Otherwise a path might try to pass
                // through it uselessly.
                let penalty = Duration::seconds(10.0);
                let sidewalk = self.nodes.get(WalkingNode::SidewalkEndpoint(
                    lane.get_directed_parent(),
                    endpt,
                ));
                input_graph.add_edge(sidewalk, ride_bus, round(cost + penalty));
                input_graph.add_edge(ride_bus, sidewalk, round(cost + penalty));
            }
        }

        // Connect each adjacent stop along a route, with the cost based on how long it'll take a
        // bus to drive between the stops. Optimistically assume no waiting time at a stop.
        for route in map.all_bus_routes() {
            // TODO Also plug in border starts
            for pair in route.stops.windows(2) {
                let (stop1, stop2) = (map.get_bs(pair[0]), map.get_bs(pair[1]));
                let req =
                    PathRequest::vehicle(stop1.driving_pos, stop2.driving_pos, route.route_type);
                let maybe_driving_cost = match route.route_type {
                    PathConstraints::Bus => bus_graph.pathfind(req, map).map(|p| p.get_cost()),
                    // We always use Dijkstra for trains
                    PathConstraints::Train => {
                        dijkstra::pathfind(req, map.routing_params(), map).map(|p| p.get_cost())
                    }
                    _ => unreachable!(),
                };
                if let Some(driving_cost) = maybe_driving_cost {
                    input_graph.add_edge(
                        self.nodes.get(WalkingNode::RideBus(stop1.id)),
                        self.nodes.get(WalkingNode::RideBus(stop2.id)),
                        round(driving_cost),
                    );
                } else {
                    panic!(
                        "No bus route from {} to {} now for {}! Prevent this edit",
                        stop1.driving_pos, stop2.driving_pos, route.full_name,
                    );
                }
            }

            if let Some(l) = route.end_border {
                let stop1 = map.get_bs(*route.stops.last().unwrap());
                let req = PathRequest::vehicle(
                    stop1.driving_pos,
                    Position::end(l, map),
                    route.route_type,
                );
                let maybe_driving_cost = match route.route_type {
                    PathConstraints::Bus => bus_graph.pathfind(req, map).map(|p| p.get_cost()),
                    // We always use Dijkstra for trains
                    PathConstraints::Train => {
                        dijkstra::pathfind(req, map.routing_params(), map).map(|p| p.get_cost())
                    }
                    _ => unreachable!(),
                };
                if let Some(driving_cost) = maybe_driving_cost {
                    let border = map.get_i(map.get_l(l).dst_i);
                    input_graph.add_edge(
                        self.nodes.get(WalkingNode::RideBus(stop1.id)),
                        self.nodes.get(WalkingNode::LeaveMap(border.id)),
                        round(driving_cost),
                    );
                } else {
                    panic!(
                        "No bus route from {} to end of {} now for {}! Prevent this edit",
                        stop1.driving_pos, l, route.full_name,
                    );
                }
            }
        }
    }

    pub fn reconstruct_path(
        &self,
        raw_nodes: &Vec<usize>,
        raw_weight: usize,
        req: PathRequest,
        map: &Map,
    ) -> PathV2 {
        let nodes: Vec<WalkingNode> = raw_nodes
            .into_iter()
            .map(|id| self.nodes.translate_id(*id))
            .collect();
        let steps = walking_path_to_steps(nodes, map);
        let cost = unround(raw_weight);
        PathV2::new(steps, req, cost, Vec::new())
    }
}

impl SidewalkPathfinder {
    pub fn new(map: &Map, use_transit: bool, bus_graph: &VehiclePathfinder) -> SidewalkPathfinder {
        let translator = if use_transit {
            SidewalkPathTranslator::walking_with_transit(map)
        } else {
            SidewalkPathTranslator::just_walking(map)
        };

        let graph = fast_paths::prepare(&translator.make_input_graph(map, Some(bus_graph)));
        SidewalkPathfinder {
            translator,
            graph,
            path_calc: ThreadLocal::new(),
        }
    }

    pub fn apply_edits(&mut self, map: &Map, bus_graph: &VehiclePathfinder) {
        let input_graph = self.translator.make_input_graph(map, Some(bus_graph));
        let node_ordering = self.graph.get_node_ordering();
        self.graph = fast_paths::prepare_with_order(&input_graph, &node_ordering).unwrap();
    }

    pub fn pathfind(&self, req: PathRequest, map: &Map) -> Option<PathV2> {
        if req.start.lane() == req.end.lane() {
            return Some(one_step_walking_path(req, map));
        }

        let mut calc = self
            .path_calc
            .get_or(|| RefCell::new(fast_paths::create_calculator(&self.graph)))
            .borrow_mut();
        let raw_path = calc.calc_path(
            &self.graph,
            self.translator
                .nodes
                .get(WalkingNode::closest(req.start, map)),
            self.translator
                .nodes
                .get(WalkingNode::closest(req.end, map)),
        )?;
        Some(self.translator.reconstruct_path(
            raw_path.get_nodes(),
            raw_path.get_weight(),
            req,
            map,
        ))
    }

    /// Attempt the pathfinding and see if we should ride a bus. If so, says (stop1, optional stop
    /// 2, route). If there's no stop 2, then ride the bus off the border.
    // TODO Lift to be useful in Dijkstra as well
    pub fn should_use_transit(
        &self,
        map: &Map,
        start: Position,
        end: Position,
    ) -> Option<(BusStopID, Option<BusStopID>, BusRouteID)> {
        let raw_path = fast_paths::calc_path(
            &self.graph,
            self.translator.nodes.get(WalkingNode::closest(start, map)),
            self.translator
                .nodes
                .get(WalkingNode::end_transit(end, map)),
        )?;

        let nodes: Vec<WalkingNode> = raw_path
            .get_nodes()
            .into_iter()
            .map(|id| self.translator.nodes.translate_id(*id))
            .collect();
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
}

// TODO Fold into reconstruct_path?
fn walking_path_to_steps(path: Vec<WalkingNode>, map: &Map) -> Vec<PathStepV2> {
    let mut steps = Vec::new();

    for pair in path.windows(2) {
        let (r1, r1_endpt) = match pair[0] {
            WalkingNode::SidewalkEndpoint(r, endpt) => (r, endpt),
            WalkingNode::RideBus(_) => unreachable!(),
            WalkingNode::LeaveMap(_) => unreachable!(),
        };
        let r2 = match pair[1] {
            WalkingNode::SidewalkEndpoint(r, _) => r,
            WalkingNode::RideBus(_) => unreachable!(),
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
            if map
                .get_turn_between(r1.must_get_sidewalk(map), r2.must_get_sidewalk(map), i)
                .is_some()
            {
                steps.push(PathStepV2::Movement(MovementID {
                    from: r1,
                    to: r2,
                    parent: i,
                    crosswalk: true,
                }));
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
    if let PathStepV2::Movement(mvmnt) = steps[0] {
        if mvmnt.from.src_i(map) == mvmnt.parent {
            steps.insert(0, PathStepV2::Contraflow(mvmnt.from));
        } else {
            steps.insert(0, PathStepV2::Along(mvmnt.from));
        }
    }
    if let PathStepV2::Movement(mvmnt) = steps.last().cloned().unwrap() {
        if mvmnt.to.src_i(map) == mvmnt.parent {
            steps.push(PathStepV2::Along(mvmnt.to));
        } else {
            steps.push(PathStepV2::Contraflow(mvmnt.to));
        }
    }

    steps
}

// TODO Do we even need this at all?
pub fn one_step_walking_path(req: PathRequest, map: &Map) -> PathV2 {
    let l = req.start.lane();
    // Weird case, but it can happen for walking from a building path to a bus stop that're
    // actually at the same spot.
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
    PathV2::new(vec![step_v2], req, cost, Vec::new())
}

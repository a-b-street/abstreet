use crate::{
    BusRouteID, BusStopID, LaneID, LaneType, Map, Path, PathRequest, PathStep, Position,
    Traversable, TurnID,
};
use geom::{Distance, Pt2D};
use ordered_float::NotNan;
use std::collections::{BinaryHeap, HashMap};

// Returns an inclusive path, aka, [start, ..., end]
pub fn shortest_distance(map: &Map, req: PathRequest) -> Option<Path> {
    // TODO using first_pt here and in heuristic_dist is particularly bad for walking
    // directions
    let goal_pt = req.end.pt(map);
    let internal_steps = SlowPathfinder {
        goal_pt,
        can_use_bike_lanes: req.can_use_bike_lanes,
        can_use_bus_lanes: req.can_use_bus_lanes,
        can_use_transit: false,
    }
    .pathfind(map, req.start, req.end)?;
    let steps: Vec<PathStep> = internal_steps
        .into_iter()
        .map(|s| match s {
            InternalPathStep::Lane(l) => PathStep::Lane(l),
            InternalPathStep::ContraflowLane(l) => PathStep::ContraflowLane(l),
            InternalPathStep::Turn(t) => PathStep::Turn(t),
            InternalPathStep::RideBus(_, _, _) => {
                panic!("shortest_distance pathfind had {:?} as a step", s)
            }
        })
        .collect();
    assert_eq!(
        steps[0].as_traversable(),
        Traversable::Lane(req.start.lane())
    );
    assert_eq!(
        steps.last().unwrap().as_traversable(),
        Traversable::Lane(req.end.lane())
    );
    Some(Path::new(map, steps, req.end.dist_along()))
}

// Attempt the pathfinding and see if riding a bus is a step.
pub fn should_use_transit(
    map: &Map,
    start: Position,
    goal: Position,
) -> Option<(BusStopID, BusStopID, BusRouteID)> {
    // TODO using first_pt here and in heuristic_dist is particularly bad for walking
    // directions
    let goal_pt = goal.pt(map);
    let internal_steps = SlowPathfinder {
        goal_pt,
        can_use_bike_lanes: false,
        can_use_bus_lanes: false,
        can_use_transit: true,
    }
    .pathfind(map, start, goal)?;
    for s in internal_steps.into_iter() {
        if let InternalPathStep::RideBus(stop1, stop2, route) = s {
            return Some((stop1, stop2, route));
        }
    }
    None
}

// TODO This is like PathStep, but also encodes the possibility of taking a bus.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
enum InternalPathStep {
    Lane(LaneID),
    ContraflowLane(LaneID),
    Turn(TurnID),
    RideBus(BusStopID, BusStopID, BusRouteID),
}

impl InternalPathStep {
    // TODO Should consider the last step too... RideBus then Lane probably won't cross the full
    // lane.
    fn cost(&self, map: &Map) -> Distance {
        match *self {
            InternalPathStep::Lane(l) | InternalPathStep::ContraflowLane(l) => {
                map.get_l(l).length()
            }
            InternalPathStep::Turn(t) => map.get_t(t).geom.length(),
            // Free! For now.
            InternalPathStep::RideBus(_, _, _) => Distance::ZERO,
        }
    }

    fn heuristic(&self, goal_pt: Pt2D, map: &Map) -> Distance {
        let pt = match *self {
            InternalPathStep::Lane(l) => map.get_l(l).last_pt(),
            InternalPathStep::ContraflowLane(l) => map.get_l(l).first_pt(),
            InternalPathStep::Turn(t) => map.get_t(t).geom.last_pt(),
            InternalPathStep::RideBus(_, stop2, _) => map.get_bs(stop2).sidewalk_pos.pt(map),
        };
        pt.dist_to(goal_pt)
    }
}

struct SlowPathfinder {
    goal_pt: Pt2D,
    can_use_bike_lanes: bool,
    can_use_bus_lanes: bool,
    can_use_transit: bool,
}

impl SlowPathfinder {
    fn expand(&self, map: &Map, current: InternalPathStep) -> Vec<InternalPathStep> {
        let mut results: Vec<InternalPathStep> = Vec::new();
        match current {
            InternalPathStep::Lane(l) | InternalPathStep::ContraflowLane(l) => {
                let endpoint = if current == InternalPathStep::Lane(l) {
                    map.get_l(l).dst_i
                } else {
                    map.get_l(l).src_i
                };
                for (turn, next) in map.get_next_turns_and_lanes(l, endpoint).into_iter() {
                    if !map.is_turn_allowed(turn.id) {
                        // Skip
                    } else if !self.can_use_bike_lanes && next.lane_type == LaneType::Biking {
                        // Skip
                    } else if !self.can_use_bus_lanes && next.lane_type == LaneType::Bus {
                        // Skip
                    } else {
                        results.push(InternalPathStep::Turn(turn.id));
                    }
                }

                if self.can_use_transit {
                    for stop1 in &map.get_l(l).bus_stops {
                        for (stop2, route) in map.get_connected_bus_stops(*stop1).into_iter() {
                            results.push(InternalPathStep::RideBus(*stop1, stop2, route));
                        }
                    }
                }
            }
            InternalPathStep::Turn(t) => {
                let dst = map.get_l(t.dst);
                if t.parent == dst.src_i {
                    results.push(InternalPathStep::Lane(dst.id));
                } else {
                    results.push(InternalPathStep::ContraflowLane(dst.id));
                }

                // Don't forget multiple turns in a row.
                for (turn, next) in map.get_next_turns_and_lanes(dst.id, t.parent).into_iter() {
                    if !map.is_turn_allowed(turn.id) {
                        // Skip
                    } else if !self.can_use_bike_lanes && next.lane_type == LaneType::Biking {
                        // Skip
                    } else if !self.can_use_bus_lanes && next.lane_type == LaneType::Bus {
                        // Skip
                    } else {
                        results.push(InternalPathStep::Turn(turn.id));
                    }
                }
            }
            InternalPathStep::RideBus(_, stop2, _) => {
                let pos = map.get_bs(stop2).sidewalk_pos;
                let sidewalk = map.get_l(pos.lane());
                if pos.dist_along() != sidewalk.length() {
                    results.push(InternalPathStep::Lane(sidewalk.id));
                }
                if pos.dist_along() != Distance::ZERO {
                    results.push(InternalPathStep::ContraflowLane(sidewalk.id));
                }
            }
        };
        results
    }

    fn pathfind(&self, map: &Map, start: Position, end: Position) -> Option<Vec<InternalPathStep>> {
        if start.lane() == end.lane() {
            if start.dist_along() > end.dist_along() {
                if !map.get_l(start.lane()).is_sidewalk() {
                    panic!("Pathfinding request with start > end dist, for non-sidewalks. start {:?} and end {:?}", start, end);
                }
                return Some(vec![InternalPathStep::ContraflowLane(start.lane())]);
            }
            return Some(vec![InternalPathStep::Lane(start.lane())]);
        }

        // This should be deterministic, since cost ties would be broken by PathStep.
        let mut queue: BinaryHeap<(NotNan<f64>, InternalPathStep)> = BinaryHeap::new();
        let start_len = map.get_l(start.lane()).length();
        if map.get_l(start.lane()).is_sidewalk() {
            if start.dist_along() != start_len {
                let step = InternalPathStep::Lane(start.lane());
                let cost = start_len - start.dist_along();
                let heuristic = step.heuristic(self.goal_pt, map);
                queue.push((dist_to_pri_queue(cost + heuristic), step));
            }
            if start.dist_along() != Distance::ZERO {
                let step = InternalPathStep::ContraflowLane(start.lane());
                let cost = start.dist_along();
                let heuristic = step.heuristic(self.goal_pt, map);
                queue.push((dist_to_pri_queue(cost + heuristic), step));
            }
        } else {
            let step = InternalPathStep::Lane(start.lane());
            let cost = start_len - start.dist_along();
            let heuristic = step.heuristic(self.goal_pt, map);
            queue.push((dist_to_pri_queue(cost + heuristic), step));
        }

        let mut backrefs: HashMap<InternalPathStep, InternalPathStep> = HashMap::new();

        while !queue.is_empty() {
            let (cost_sofar, current) = queue.pop().unwrap();

            // Found it, now produce the path
            if current == InternalPathStep::Lane(end.lane())
                || current == InternalPathStep::ContraflowLane(end.lane())
            {
                let mut reversed_steps: Vec<InternalPathStep> = Vec::new();
                let mut lookup = current;
                loop {
                    reversed_steps.push(lookup);
                    if lookup == InternalPathStep::Lane(start.lane())
                        || lookup == InternalPathStep::ContraflowLane(start.lane())
                    {
                        reversed_steps.reverse();
                        return Some(reversed_steps);
                    }
                    lookup = backrefs[&lookup];
                }
            }

            // Expand
            for next in self.expand(map, current).into_iter() {
                backrefs.entry(next).or_insert_with(|| {
                    let cost = next.cost(map);
                    let heuristic = next.heuristic(self.goal_pt, map);
                    queue.push((dist_to_pri_queue(cost + heuristic) + cost_sofar, next));

                    current
                });
            }
        }

        // No path
        None
    }
}

// Negate since BinaryHeap is a max-heap.
fn dist_to_pri_queue(dist: Distance) -> NotNan<f64> {
    NotNan::new(-dist.inner_meters()).unwrap()
}

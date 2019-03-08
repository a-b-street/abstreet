use crate::{LaneType, Map, Path, PathRequest, PathStep, Position, Traversable};
use geom::{Distance, Pt2D};
use ordered_float::NotNan;
use std::collections::{BinaryHeap, HashMap};

// Only for vehicle paths, no walking support.
pub fn shortest_distance(map: &Map, req: PathRequest) -> Option<Path> {
    // TODO using first_pt here and in heuristic_dist is particularly bad for walking
    // directions
    let goal_pt = req.end.pt(map);
    let steps = SlowPathfinder {
        goal_pt,
        can_use_bike_lanes: req.can_use_bike_lanes,
        can_use_bus_lanes: req.can_use_bus_lanes,
    }
    .pathfind(map, req.start, req.end)?;
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

struct SlowPathfinder {
    goal_pt: Pt2D,
    can_use_bike_lanes: bool,
    can_use_bus_lanes: bool,
}

impl SlowPathfinder {
    fn expand(&self, map: &Map, current: PathStep) -> Vec<PathStep> {
        let mut results: Vec<PathStep> = Vec::new();
        match current {
            PathStep::Lane(l) => {
                for (turn, next) in map
                    .get_next_turns_and_lanes(l, map.get_l(l).dst_i)
                    .into_iter()
                {
                    if !map.is_turn_allowed(turn.id) {
                        // Skip
                    } else if !self.can_use_bike_lanes && next.lane_type == LaneType::Biking {
                        // Skip
                    } else if !self.can_use_bus_lanes && next.lane_type == LaneType::Bus {
                        // Skip
                    } else {
                        results.push(PathStep::Turn(turn.id));
                    }
                }
            }
            PathStep::Turn(t) => {
                results.push(PathStep::Lane(t.dst));
            }
            PathStep::ContraflowLane(_) => unreachable!(),
        };
        results
    }

    fn pathfind(&self, map: &Map, start: Position, end: Position) -> Option<Vec<PathStep>> {
        // This should be deterministic, since cost ties would be broken by PathStep.
        let mut queue: BinaryHeap<(NotNan<f64>, PathStep)> = BinaryHeap::new();
        {
            let step = PathStep::Lane(start.lane());
            let cost = map.get_l(start.lane()).length() - start.dist_along();
            let heuristic = heuristic(&step, self.goal_pt, map);
            queue.push((dist_to_pri_queue(cost + heuristic), step));
        }

        let mut backrefs: HashMap<PathStep, PathStep> = HashMap::new();

        while !queue.is_empty() {
            let (cost_sofar, current) = queue.pop().unwrap();

            // Found it, now produce the path
            if current == PathStep::Lane(end.lane()) {
                let mut reversed_steps: Vec<PathStep> = Vec::new();
                let mut lookup = current;
                loop {
                    reversed_steps.push(lookup);
                    if lookup == PathStep::Lane(start.lane()) {
                        reversed_steps.reverse();
                        return Some(reversed_steps);
                    }
                    lookup = backrefs[&lookup];
                }
            }

            // Expand
            for next in self.expand(map, current).into_iter() {
                backrefs.entry(next).or_insert_with(|| {
                    let cost = cost(&next, map);
                    let heuristic = heuristic(&next, self.goal_pt, map);
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

fn cost(step: &PathStep, map: &Map) -> Distance {
    match step {
        PathStep::Lane(l) => map.get_l(*l).length(),
        PathStep::Turn(t) => map.get_t(*t).geom.length(),
        PathStep::ContraflowLane(_) => unreachable!(),
    }
}

fn heuristic(step: &PathStep, goal_pt: Pt2D, map: &Map) -> Distance {
    let pt = match step {
        PathStep::Lane(l) => map.get_l(*l).last_pt(),
        PathStep::Turn(t) => map.get_t(*t).geom.last_pt(),
        PathStep::ContraflowLane(_) => unreachable!(),
    };
    pt.dist_to(goal_pt)
}

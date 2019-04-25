use counter::Counter;
use map_model::{IntersectionID, LaneID, PathStep};
use sim::Sim;
use std::collections::HashSet;

const TOP_N: usize = 10;

pub struct ChokepointsFinder {
    pub lanes: HashSet<LaneID>,
    pub intersections: HashSet<IntersectionID>,
}

impl ChokepointsFinder {
    pub fn new(sim: &Sim) -> ChokepointsFinder {
        let mut count_per_lane: Counter<LaneID, usize> = Counter::new();
        let mut count_per_intersection: Counter<IntersectionID, usize> = Counter::new();

        let active = sim.active_agents();
        println!("Finding chokepoints from {} active agents", active.len());
        for a in active.into_iter() {
            // Why would an active agent not have a path? Pedestrian riding a bus.
            if let Some(path) = sim.get_path(a) {
                for step in path.get_steps() {
                    match step {
                        PathStep::Lane(l) | PathStep::ContraflowLane(l) => {
                            count_per_lane.update(vec![*l]);
                        }
                        PathStep::Turn(t) => {
                            count_per_intersection.update(vec![t.parent]);
                        }
                    }
                }
            }
        }

        let lanes: HashSet<LaneID> = count_per_lane
            .most_common_ordered()
            .into_iter()
            .take(TOP_N)
            .map(|(l, _)| l)
            .collect();
        let intersections: HashSet<IntersectionID> = count_per_intersection
            .most_common_ordered()
            .into_iter()
            .take(TOP_N)
            .map(|(i, _)| i)
            .collect();
        ChokepointsFinder {
            lanes,
            intersections,
        }
    }
}

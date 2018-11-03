use dimensioned::si;
use geom::{Line, Pt2D};
use ordered_float::NotNaN;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use {LaneID, LaneType, Map, Traversable, TurnID};

// TODO Make copy and return copies from all the Path queries, so we can stop dereferencing
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum PathStep {
    // Original direction
    Lane(LaneID),
    // Sidewalks only!
    ContraflowLane(LaneID),
    Turn(TurnID),
}

// TODO All of these feel a bit hacky.
impl PathStep {
    pub fn is_contraflow(&self) -> bool {
        match self {
            PathStep::ContraflowLane(_) => true,
            _ => false,
        }
    }

    pub fn as_traversable(&self) -> Traversable {
        match self {
            PathStep::Lane(id) => Traversable::Lane(*id),
            PathStep::ContraflowLane(id) => Traversable::Lane(*id),
            PathStep::Turn(id) => Traversable::Turn(*id),
        }
    }

    pub fn as_turn(&self) -> TurnID {
        self.as_traversable().as_turn()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Path {
    // TODO way to encode start/end dist? I think it's needed for trace_route later...
    // actually not start dist -- that really changes all the time
    steps: VecDeque<PathStep>,
}

// TODO can have a method to verify the path is valid
impl Path {
    fn new(steps: Vec<PathStep>) -> Path {
        Path {
            steps: VecDeque::from(steps),
        }
    }

    pub fn num_lanes(&self) -> usize {
        let mut count = 0;
        for s in &self.steps {
            match s {
                PathStep::Lane(_) | PathStep::ContraflowLane(_) => count += 1,
                _ => {}
            };
        }
        count
    }

    pub fn is_last_step(&self) -> bool {
        self.steps.len() == 1
    }

    pub fn isnt_last_step(&self) -> bool {
        self.steps.len() > 1
    }

    pub fn shift(&mut self) -> PathStep {
        self.steps.pop_front().unwrap()
    }

    pub fn add(&mut self, step: PathStep) {
        self.steps.push_back(step);
    }

    pub fn current_step(&self) -> &PathStep {
        &self.steps[0]
    }

    pub fn next_step(&self) -> &PathStep {
        &self.steps[1]
    }

    pub fn last_step(&self) -> &PathStep {
        &self.steps[self.steps.len() - 1]
    }
}

pub enum Pathfinder {
    ShortestDistance { goal_pt: Pt2D, is_bike: bool },
    // TODO result isn't really lanes, we also want to know bus stops... post-process? remember
    // more stuff? hmm.
    UsingTransit,
}

impl Pathfinder {
    // Returns an inclusive path, aka, [start, ..., end]
    pub fn shortest_distance(
        map: &Map,
        start: LaneID,
        start_dist: si::Meter<f64>,
        end: LaneID,
        end_dist: si::Meter<f64>,
        is_bike: bool,
    ) -> Option<Path> {
        // TODO using first_pt here and in heuristic_dist is particularly bad for walking
        // directions
        let goal_pt = map.get_l(end).first_pt();
        Pathfinder::ShortestDistance { goal_pt, is_bike }
            .pathfind(map, start, start_dist, end, end_dist)
    }

    fn expand(&self, map: &Map, current: LaneID) -> Vec<(LaneID, NotNaN<f64>)> {
        match self {
            Pathfinder::ShortestDistance { goal_pt, is_bike } => {
                let current_length = NotNaN::new(map.get_l(current).length().value_unsafe).unwrap();
                map.get_next_turns_and_lanes(current)
                    .into_iter()
                    .filter_map(|(_turn, next)| {
                        if !is_bike && next.lane_type == LaneType::Biking {
                            None
                        } else {
                            let heuristic_dist = NotNaN::new(
                                Line::new(next.first_pt(), *goal_pt).length().value_unsafe,
                            ).unwrap();
                            Some((next.id, current_length + heuristic_dist))
                        }
                    }).collect()
            }
            Pathfinder::UsingTransit => {
                // No heuristic, because it's hard to make admissible.
                // Cost is distance spent walking, so any jumps made using a bus are FREE. This is
                // unrealistic, but a good way to start exercising peds using transit.
                let current_lane = map.get_l(current);
                let current_length = NotNaN::new(current_lane.length().value_unsafe).unwrap();
                let mut results: Vec<(LaneID, NotNaN<f64>)> = Vec::new();
                for (_turn, next) in &map.get_next_turns_and_lanes(current) {
                    results.push((next.id, current_length));
                }
                for stop1 in &current_lane.bus_stops {
                    for stop2 in &map.get_connected_bus_stops(*stop1) {
                        results.push((stop2.sidewalk, current_length));
                    }
                }
                results
            }
        }
    }

    fn pathfind(
        &self,
        map: &Map,
        start: LaneID,
        start_dist: si::Meter<f64>,
        end: LaneID,
        end_dist: si::Meter<f64>,
    ) -> Option<Path> {
        assert_eq!(map.get_l(start).lane_type, map.get_l(end).lane_type);
        if start == end {
            if start_dist > end_dist {
                assert_eq!(map.get_l(start).lane_type, LaneType::Sidewalk);
                return Some(Path::new(vec![PathStep::ContraflowLane(start)]));
            }
            return Some(Path::new(vec![PathStep::Lane(start)]));
        }

        // This should be deterministic, since cost ties would be broken by LaneID.
        let mut queue: BinaryHeap<(NotNaN<f64>, LaneID)> = BinaryHeap::new();
        queue.push((NotNaN::new(-0.0).unwrap(), start));

        let mut backrefs: HashMap<LaneID, LaneID> = HashMap::new();

        while !queue.is_empty() {
            let (cost_sofar, current) = queue.pop().unwrap();

            // Found it, now produce the path
            if current == end {
                let mut steps: VecDeque<PathStep> = VecDeque::new();
                let mut lookup = current;
                loop {
                    steps.push_front(PathStep::Lane(lookup));
                    if lookup == start {
                        assert_eq!(steps[0], PathStep::Lane(start));
                        assert_eq!(*steps.back().unwrap(), PathStep::Lane(end));
                        return Some(Path { steps });
                    }
                    lookup = backrefs[&lookup];
                }
            }

            // Expand
            for (next, cost) in self.expand(map, current).into_iter() {
                if !backrefs.contains_key(&next) {
                    backrefs.insert(next, current);
                    // Negate since BinaryHeap is a max-heap.
                    queue.push((NotNaN::new(-1.0).unwrap() * (cost + cost_sofar), next));
                }
            }
        }

        // No path
        None
    }
}

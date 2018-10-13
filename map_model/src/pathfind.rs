use geom::{Line, Pt2D};
use ordered_float::NotNaN;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use {LaneID, LaneType, Map};

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
        end: LaneID,
        is_bike: bool,
    ) -> Option<VecDeque<LaneID>> {
        let goal_pt = map.get_l(end).first_pt();
        Pathfinder::ShortestDistance { goal_pt, is_bike }.pathfind(map, start, end)
    }

    fn expand(&self, map: &Map, current: LaneID) -> Vec<(LaneID, NotNaN<f64>)> {
        match self {
            Pathfinder::ShortestDistance { goal_pt, is_bike } => {
                let current_length = NotNaN::new(map.get_l(current).length().value_unsafe).unwrap();
                map.get_next_lanes(current)
                    .iter()
                    .filter_map(|next| {
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
                for next in &map.get_next_lanes(current) {
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

    fn pathfind(&self, map: &Map, start: LaneID, end: LaneID) -> Option<VecDeque<LaneID>> {
        assert_eq!(map.get_l(start).lane_type, map.get_l(end).lane_type);
        if start == end {
            return Some(VecDeque::from(vec![start]));
        }

        // This should be deterministic, since cost ties would be broken by LaneID.
        let mut queue: BinaryHeap<(NotNaN<f64>, LaneID)> = BinaryHeap::new();
        queue.push((NotNaN::new(-0.0).unwrap(), start));

        let mut backrefs: HashMap<LaneID, LaneID> = HashMap::new();

        while !queue.is_empty() {
            let (cost_sofar, current) = queue.pop().unwrap();

            // Found it, now produce the path
            if current == end {
                let mut path: VecDeque<LaneID> = VecDeque::new();
                let mut lookup = current;
                loop {
                    path.push_front(lookup);
                    if lookup == start {
                        assert_eq!(path[0], start);
                        assert_eq!(*path.back().unwrap(), end);
                        return Some(path);
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

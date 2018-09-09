use geom::{Line, Pt2D};
use ordered_float::NotNaN;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use {LaneID, Map};

pub enum Pathfinder {
    ShortestDistance { goal_pt: Pt2D },
}

impl Pathfinder {
    // Returns an inclusive path, aka, [start, ..., end]
    pub fn shortest_distance(map: &Map, start: LaneID, end: LaneID) -> Option<VecDeque<LaneID>> {
        let goal_pt = map.get_l(end).first_pt();
        Pathfinder::ShortestDistance { goal_pt }.pathfind(map, start, end)
    }

    fn expand(&self, map: &Map, current: LaneID) -> Vec<(LaneID, NotNaN<f64>)> {
        match self {
            Pathfinder::ShortestDistance { goal_pt } => {
                let current_length = NotNaN::new(map.get_l(current).length().value_unsafe).unwrap();
                map.get_next_lanes(current)
                    .iter()
                    .map(|next| {
                        let heuristic_dist = NotNaN::new(
                            Line::new(next.first_pt(), *goal_pt).length().value_unsafe,
                        ).unwrap();
                        (next.id, current_length + heuristic_dist)
                    })
                    .collect()
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

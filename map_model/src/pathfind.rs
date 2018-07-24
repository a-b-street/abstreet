use geom::Line;
use ordered_float::NotNaN;
use std::collections::{BinaryHeap, HashMap};
use {LaneID, Map};

// Returns an inclusive path, aka, [start, ..., end]
pub fn pathfind(map: &Map, start: LaneID, end: LaneID) -> Option<Vec<LaneID>> {
    assert_ne!(start, end);
    assert_eq!(map.get_l(start).lane_type, map.get_l(end).lane_type);

    // This should be deterministic, since theoretical distance ties would be broken by LaneID.
    let mut queue: BinaryHeap<(NotNaN<f64>, LaneID)> = BinaryHeap::new();
    queue.push((NotNaN::new(-0.0).unwrap(), start));

    let mut backrefs: HashMap<LaneID, LaneID> = HashMap::new();

    let goal_pt = map.get_l(end).first_pt();
    while !queue.is_empty() {
        let (dist_sofar, current) = queue.pop().unwrap();

        // Found it, now produce the path
        if current == end {
            let mut path: Vec<LaneID> = Vec::new();
            let mut lookup = current;
            loop {
                path.push(lookup);
                if lookup == start {
                    path.reverse();
                    assert_eq!(path[0], start);
                    assert_eq!(*path.last().unwrap(), end);
                    return Some(path);
                }
                lookup = backrefs[&lookup];
            }
        }

        // Expand
        let current_length = NotNaN::new(map.get_l(current).length().value_unsafe).unwrap();
        for next in map.get_next_lanes(current) {
            if !backrefs.contains_key(&next.id) {
                backrefs.insert(next.id, current);
                let heuristic_dist =
                    NotNaN::new(Line::new(next.first_pt(), goal_pt).length().value_unsafe).unwrap();
                // Negate since BinaryHeap is a max-heap.
                let total_dist = current_length + heuristic_dist - dist_sofar;
                queue.push((-total_dist, next.id));
            }
        }
    }

    // No path
    None
}

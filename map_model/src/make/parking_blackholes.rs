use crate::{IntersectionType, LaneID, Map};
use abstutil::Timer;
use petgraph::graphmap::DiGraphMap;
use std::collections::{HashSet, VecDeque};

// Returns list of (driving lane, redirect here instead for parking)
//
// It's a bit weird to never attempt parking on roads not part of the largest SCC of the graph.
// This is acceptable, because there shouldn't be too many roads outside of that SCC anyway.
pub fn redirect_parking_blackholes(map: &Map, timer: &mut Timer) -> Vec<(LaneID, LaneID)> {
    let mut graph = DiGraphMap::new();
    for turn in map.all_turns().values() {
        if map.is_turn_allowed(turn.id) && !turn.between_sidewalks() {
            graph.add_edge(turn.id.src, turn.id.dst, 1);
        }
    }
    let components = petgraph::algo::kosaraju_scc(&graph);
    let largest_group: HashSet<LaneID> = components
        .into_iter()
        .max_by_key(|c| c.len())
        .unwrap()
        .into_iter()
        .collect();

    let mut redirects = Vec::new();
    timer.start_iter("find parking blackhole redirects", map.all_lanes().len());
    for l in map.all_lanes() {
        timer.next();
        if !l.is_driving() || largest_group.contains(&l.id) {
            continue;
        }

        // Search backwards for the nearest driving lane belonging to largest_group.
        if let Some(redirect) = reverse_flood(map, l.id, &largest_group) {
            redirects.push((l.id, redirect));
        } else {
            // If the lane starts at a border, totally expected to have no possible redirect.
            if map.get_i(l.src_i).intersection_type != IntersectionType::Border {
                // TODO Hmm, this is firing for lanes that look well-connected...
                timer.warn(format!(
                    "{} is a parking blackhole with no reasonable redirect!",
                    l.id
                ));
            }
        }
    }
    timer.note(format!(
        "{} driving lanes are parking blackholes",
        redirects.len()
    ));
    redirects
}

fn reverse_flood(map: &Map, start: LaneID, largest_group: &HashSet<LaneID>) -> Option<LaneID> {
    let mut queue = VecDeque::new();
    queue.push_back(start);
    let mut visisted = HashSet::new();

    while !queue.is_empty() {
        let current = queue.pop_front().unwrap();
        if visisted.contains(&current) {
            continue;
        }
        visisted.insert(current);
        if largest_group.contains(&current) {
            return Some(current);
        }
        for turn in map.get_turns_to_lane(current) {
            if map.is_turn_allowed(turn.id) {
                queue.push_back(turn.id.src);
            }
        }
    }
    None
}

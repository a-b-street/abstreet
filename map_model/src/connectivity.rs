use crate::{LaneID, Map};
use petgraph::graphmap::DiGraphMap;
use std::collections::HashSet;

// SCC = strongly connected component

// TODO Move make/parking_blackholes.rs logic here.
// TODO Move debug/floodfill.rs logic here.

// Returns (sidewalks in main component, disconnected sidewalks)
pub fn find_sidewalk_scc(map: &Map) -> (HashSet<LaneID>, HashSet<LaneID>) {
    let mut graph = DiGraphMap::new();
    for turn in map.all_turns().values() {
        if map.is_turn_allowed(turn.id) && turn.between_sidewalks() {
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
    let disconnected = map
        .all_lanes()
        .iter()
        .filter_map(|l| {
            if l.is_sidewalk() && !largest_group.contains(&l.id) {
                Some(l.id)
            } else {
                None
            }
        })
        .collect();
    (largest_group, disconnected)
}

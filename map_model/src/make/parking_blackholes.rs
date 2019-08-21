use crate::{LaneID, Map};
use abstutil::Timer;
use petgraph::graphmap::DiGraphMap;
use std::collections::HashSet;

// Returns list of lanes to mark as blackholes
pub fn redirect_parking_blackholes(map: &Map, timer: &mut Timer) -> Vec<LaneID> {
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

    let mut blackholes = Vec::new();
    for l in map.all_lanes() {
        if !l.is_driving() {
            continue;
        }
        if !largest_group.contains(&l.id) {
            blackholes.push(l.id);
        }
    }
    timer.note(format!(
        "{} driving lanes are parking blackholes",
        blackholes.len()
    ));
    blackholes
}

use crate::{BuildingID, LaneID, Map, PathConstraints, PathRequest};
use geom::Distance;
use petgraph::graphmap::DiGraphMap;
use std::collections::{HashMap, HashSet};

// SCC = strongly connected component

// Returns (relevant lanes in main component, disconnected relevant lanes)
pub fn find_scc(map: &Map, constraints: PathConstraints) -> (HashSet<LaneID>, HashSet<LaneID>) {
    let mut graph = DiGraphMap::new();
    for turn in map.all_turns().values() {
        if constraints.can_use(map.get_l(turn.id.src), map)
            && constraints.can_use(map.get_l(turn.id.dst), map)
        {
            graph.add_edge(turn.id.src, turn.id.dst, 1);
        }
    }
    let components = petgraph::algo::kosaraju_scc(&graph);
    if components.is_empty() {
        return (HashSet::new(), HashSet::new());
    }
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
            if constraints.can_use(l, map) && !largest_group.contains(&l.id) {
                Some(l.id)
            } else {
                None
            }
        })
        .collect();
    (largest_group, disconnected)
}

// TODO Different cost function
pub fn all_costs_from(map: &Map, start: BuildingID) -> HashMap<BuildingID, Distance> {
    let mut results = HashMap::new();
    let start = map.get_b(start).sidewalk_pos;
    // TODO This is SO inefficient. Flood out and mark off buildings as we go. Granularity of lane
    // makes more sense.
    for b in map.all_buildings() {
        if let Some(path) = map.pathfind(PathRequest {
            start,
            end: b.sidewalk_pos,
            constraints: PathConstraints::Pedestrian,
        }) {
            // TODO Distance isn't an interesting thing to show at all, we want the path cost
            // (probably in time)
            results.insert(b.id, path.total_length());
        }
    }
    results
}

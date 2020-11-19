// TODO Possibly these should be methods on Map.

use std::collections::{HashMap, HashSet};

use petgraph::graphmap::DiGraphMap;

use geom::Duration;

pub use crate::pathfind::{build_graph_for_pedestrians, driving_cost, WalkingNode};
use crate::{BuildingID, LaneID, Map, PathConstraints};

/// Calculate the srongy connected components (SCC) of the part of the map accessible by constraints
/// (ie, the graph of sidewalks or driving+bike lanes). The largest component is the "main" graph;
/// the rest is disconnected. Returns (lanes in the largest "main" component, all other disconnected
/// lanes)
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

/// Starting from one building, calculate the cost to all others. If a destination isn't reachable,
/// it won't be included in the results.
pub fn all_costs_from(map: &Map, start: BuildingID) -> HashMap<BuildingID, Duration> {
    // TODO This is hardcoded to walking; take a PathConstraints.
    let graph = build_graph_for_pedestrians(map);
    let start = WalkingNode::closest(map.get_b(start).sidewalk_pos, map);
    let cost_per_node = petgraph::algo::dijkstra(&graph, start, None, |(_, _, cost)| *cost);

    // Assign every building a cost based on which end of the sidewalk it's closest to
    // TODO We could try to get a little more accurate by accounting for the distance from that
    // end of the sidewalk to the building
    let mut results = HashMap::new();
    for b in map.all_buildings() {
        if let Some(seconds) = cost_per_node.get(&WalkingNode::closest(b.sidewalk_pos, map)) {
            results.insert(b.id, Duration::seconds(*seconds as f64));
        }
    }
    results
}

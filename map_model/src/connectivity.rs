// TODO Possibly these should be methods on Map.

use std::collections::{HashMap, HashSet};

use petgraph::graphmap::DiGraphMap;

use geom::Duration;

pub use crate::pathfind::{
    build_graph_for_pedestrians, build_graph_for_vehicles, driving_cost, WalkingNode,
};
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
/// it won't be included in the results. Ignore results greater than the time_limit away.
pub fn all_costs_from(
    map: &Map,
    start: BuildingID,
    time_limit: Duration,
    constraints: PathConstraints,
) -> HashMap<BuildingID, Duration> {
    let mut results = HashMap::new();

    if constraints == PathConstraints::Pedestrian {
        let graph = build_graph_for_pedestrians(map);
        let start = WalkingNode::closest(map.get_b(start).sidewalk_pos, map);
        let cost_per_node = petgraph::algo::dijkstra(&graph, start, None, |(_, _, cost)| *cost);

        // Assign every building a cost based on which end of the sidewalk it's closest to
        // TODO We could try to get a little more accurate by accounting for the distance from that
        // end of the sidewalk to the building
        for b in map.all_buildings() {
            if let Some(seconds) = cost_per_node.get(&WalkingNode::closest(b.sidewalk_pos, map)) {
                let duration = Duration::seconds(*seconds as f64);
                if duration <= time_limit {
                    results.insert(b.id, duration);
                }
            }
        }
    } else {
        // TODO We have a graph of LaneIDs, but mapping a building to one isn't straightforward. In
        // the common case it'll be fine, but some buildings are isolated from the graph by some
        // sidewalks.
        let mut bldg_to_lane = HashMap::new();
        for b in map.all_buildings() {
            if constraints == PathConstraints::Car {
                if let Some((pos, _)) = b.driving_connection(map) {
                    bldg_to_lane.insert(b.id, pos.lane());
                }
            } else if constraints == PathConstraints::Bike {
                if let Some((pos, _)) = b.biking_connection(map) {
                    bldg_to_lane.insert(b.id, pos.lane());
                }
            }
        }

        if let Some(start_lane) = bldg_to_lane.get(&start) {
            let graph = build_graph_for_vehicles(map, constraints);
            let cost_per_lane =
                petgraph::algo::dijkstra(&graph, *start_lane, None, |(_, _, turn)| {
                    driving_cost(map.get_l(turn.src), map.get_t(*turn), constraints, map)
                });
            for (b, lane) in bldg_to_lane {
                if let Some(seconds) = cost_per_lane.get(&lane) {
                    let duration = Duration::seconds(*seconds as f64);
                    if duration <= time_limit {
                        results.insert(b, duration);
                    }
                }
            }
        }
    }

    results
}

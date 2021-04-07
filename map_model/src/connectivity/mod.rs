// TODO Possibly these should be methods on Map.

use std::collections::{HashMap, HashSet};

use petgraph::graphmap::DiGraphMap;

use geom::Duration;

pub use self::walking::{all_walking_costs_from, WalkingOptions};
use crate::pathfind::{build_graph_for_vehicles, build_graph_for_vehicles_v2, zone_cost};
pub use crate::pathfind::{vehicle_cost, vehicle_cost_v2, WalkingNode};
use crate::{BuildingID, LaneID, Map, PathConstraints, PathRequest, RoadID};

mod walking;

/// Calculate the strongly connected components (SCC) of the part of the map accessible by
/// constraints (ie, the graph of sidewalks or driving+bike lanes). The largest component is the
/// "main" graph; the rest is disconnected. Returns (lanes in the largest "main" component, all
/// other disconnected lanes)
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
pub fn all_vehicle_costs_from(
    map: &Map,
    start: BuildingID,
    time_limit: Duration,
    constraints: PathConstraints,
) -> HashMap<BuildingID, Duration> {
    assert!(constraints != PathConstraints::Pedestrian);
    let mut results = HashMap::new();

    // TODO We have a graph of DirectedRoadIDs, but mapping a building to one isn't
    // straightforward. In the common case it'll be fine, but some buildings are isolated from the
    // graph by some sidewalks.
    let mut bldg_to_road = HashMap::new();
    for b in map.all_buildings() {
        if constraints == PathConstraints::Car {
            if let Some((pos, _)) = b.driving_connection(map) {
                bldg_to_road.insert(b.id, map.get_l(pos.lane()).get_directed_parent());
            }
        } else if constraints == PathConstraints::Bike {
            if let Some((pos, _)) = b.biking_connection(map) {
                bldg_to_road.insert(b.id, map.get_l(pos.lane()).get_directed_parent());
            }
        }
    }

    if let Some(start_road) = bldg_to_road.get(&start) {
        let graph = build_graph_for_vehicles_v2(map, constraints);
        let cost_per_road = petgraph::algo::dijkstra(&graph, *start_road, None, |(_, _, mvmnt)| {
            vehicle_cost_v2(mvmnt.from, *mvmnt, constraints, map.routing_params(), map)
        });
        for (b, road) in bldg_to_road {
            if let Some(duration) = cost_per_road.get(&road).cloned() {
                if duration <= time_limit {
                    results.insert(b, duration);
                }
            }
        }
    }

    results
}

// TODO Refactor with all_vehicle_costs_from
pub fn debug_vehicle_costs(
    req: PathRequest,
    map: &Map,
) -> Option<(Duration, HashMap<RoadID, Duration>)> {
    // TODO Support this
    if req.constraints == PathConstraints::Pedestrian {
        return None;
    }
    let graph = build_graph_for_vehicles(map, req.constraints);

    let (cost, _) = petgraph::algo::astar(
        &graph,
        req.start.lane(),
        |l| l == req.end.lane(),
        |(_, _, t)| {
            let turn = map.get_t(*t);
            vehicle_cost(
                map.get_l(turn.id.src),
                turn,
                req.constraints,
                map.routing_params(),
                map,
            ) + zone_cost(turn, req.constraints, map)
        },
        |_| Duration::ZERO,
    )?;

    let lane_costs = petgraph::algo::dijkstra(&graph, req.start.lane(), None, |(_, _, t)| {
        let turn = map.get_t(*t);
        vehicle_cost(
            map.get_l(turn.id.src),
            turn,
            req.constraints,
            map.routing_params(),
            map,
        ) + zone_cost(turn, req.constraints, map)
    });
    // Express the costs per road for an easier debug experince. Take the LOWEST cost per road,
    // since we don't want noise from considering the opposite direction.
    let mut road_costs = HashMap::new();
    for (l, cost) in lane_costs {
        let road_cost = road_costs.entry(map.get_l(l).parent).or_insert(cost);
        *road_cost = (*road_cost).min(cost);
    }

    Some((cost, road_costs))
}

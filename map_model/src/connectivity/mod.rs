// TODO Possibly these should be methods on Map.

use std::collections::{HashMap, HashSet};

use petgraph::graphmap::DiGraphMap;

use geom::{Distance, Duration, Speed};

pub use self::walking::{all_walking_costs_from, WalkingOptions};
use crate::pathfind::{build_graph_for_vehicles, zone_cost};
pub use crate::pathfind::{vehicle_cost, WalkingNode};
use crate::{BuildingID, LaneID, Map, PathConstraints, PathRequest, RoadID};

mod walking;

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
pub fn all_vehicle_costs_from(
    map: &Map,
    start: BuildingID,
    time_limit: Duration,
    constraints: PathConstraints,
) -> HashMap<BuildingID, Duration> {
    assert!(constraints != PathConstraints::Pedestrian);
    let mut results = HashMap::new();

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

    // TODO Copied from simulation code :(
    let max_bike_speed = Speed::miles_per_hour(10.0);

    if let Some(start_lane) = bldg_to_lane.get(&start) {
        let graph = build_graph_for_vehicles(map, constraints);
        let cost_per_lane = petgraph::algo::dijkstra(&graph, *start_lane, None, |(_, _, turn)| {
            vehicle_cost(
                map.get_l(turn.src),
                map.get_t(*turn),
                constraints,
                map.routing_params(),
                map,
            )
        });
        for (b, lane) in bldg_to_lane {
            if let Some(meters) = cost_per_lane.get(&lane) {
                let distance = Distance::meters(*meters as f64);
                let duration = distance / max_bike_speed;
                if duration <= time_limit {
                    results.insert(b, duration);
                }
            }
        }
    }

    results
}

// TODO Refactor with all_vehicle_costs_from
pub fn debug_vehicle_costs(req: PathRequest, map: &Map) -> Option<(f64, HashMap<RoadID, f64>)> {
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
        |_| 0.0,
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
        *road_cost = road_cost.min(cost);
    }

    Some((cost, road_costs))
}

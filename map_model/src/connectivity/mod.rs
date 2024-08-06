// TODO Possibly these should be methods on Map.

use abstutil::MultiMap;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet};

use petgraph::graphmap::DiGraphMap;

use abstutil::PriorityQueueItem;
use geom::Duration;

pub use self::walking::{all_walking_costs_from, WalkingOptions};
pub use crate::pathfind::{vehicle_cost, WalkingNode};
use crate::{
    Building, BuildingID, DirectedRoadID, IntersectionID, LaneID, Map, PathConstraints, RoadID,
};

mod walking;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Spot {
    Building(BuildingID),
    Border(IntersectionID),
    DirectedRoad(DirectedRoadID),
}

/// Calculate the strongly connected components (SCC) of the part of the map accessible by
/// constraints (ie, the graph of sidewalks or driving+bike lanes). The largest component is the
/// "main" graph; the rest is disconnected. Returns (lanes in the largest "main" component, all
/// other disconnected lanes)
pub fn find_scc(map: &Map, constraints: PathConstraints) -> (HashSet<LaneID>, HashSet<LaneID>) {
    let mut graph = DiGraphMap::new();
    for turn in map.all_turns() {
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

fn bldg_to_dir_road(
    map: &Map,
    b: &Building,
    constraints: PathConstraints,
) -> Option<DirectedRoadID> {
    let pos = if constraints == PathConstraints::Car {
        b.driving_connection(map)?.0
    } else if constraints == PathConstraints::Bike {
        b.biking_connection(map)?.0
    } else {
        return None;
    };
    Some(map.get_l(pos.lane()).get_directed_parent())
}

/// Starting from some initial spot, calculate the cost to all buildings. If a destination isn't
/// reachable, it won't be included in the results. Ignore results greater than the time_limit
/// away.
///
/// Costs for roads will only be filled out for roads with no buildings along them. The cost will
/// be the same for the entire road, which may be misleading for long roads.
pub fn all_vehicle_costs_from(
    map: &Map,
    starts: Vec<Spot>,
    time_limit: Duration,
    constraints: PathConstraints,
) -> (HashMap<BuildingID, Duration>, HashMap<RoadID, Duration>) {
    assert!(constraints != PathConstraints::Pedestrian);
    // TODO We have a graph of DirectedRoadIDs, but mapping a building to one isn't
    // straightforward. In the common case it'll be fine, but some buildings are isolated from the
    // graph by some sidewalks.

    let mut dir_road_to_bldgs = MultiMap::new();
    for b in map.all_buildings() {
        if let Some(dr) = bldg_to_dir_road(map, b, constraints) {
            dir_road_to_bldgs.insert(dr, b.id);
        }
    }

    let mut queue: BinaryHeap<PriorityQueueItem<Duration, DirectedRoadID>> = BinaryHeap::new();

    for spot in starts {
        match spot {
            Spot::Building(b_id) => {
                if let Some(start_road) = bldg_to_dir_road(map, map.get_b(b_id), constraints) {
                    queue.push(PriorityQueueItem {
                        cost: Duration::ZERO,
                        value: start_road,
                    });
                }
            }
            Spot::Border(i_id) => {
                let intersection = map.get_i(i_id);

                let incoming_lanes = intersection.get_incoming_lanes(map, constraints);
                let mut outgoing_lanes = intersection.get_outgoing_lanes(map, constraints);
                let mut all_lanes = incoming_lanes;
                all_lanes.append(&mut outgoing_lanes);

                for l_id in all_lanes {
                    queue.push(PriorityQueueItem {
                        cost: Duration::ZERO,
                        value: map.get_l(l_id).get_directed_parent(),
                    });
                }
            }
            Spot::DirectedRoad(dr) => {
                queue.push(PriorityQueueItem {
                    cost: Duration::ZERO,
                    value: dr,
                });
            }
        }
    }

    let mut visited_nodes = HashSet::new();
    let mut bldg_results = HashMap::new();
    let mut road_results = HashMap::new();

    while let Some(current) = queue.pop() {
        if visited_nodes.contains(&current.value) {
            continue;
        }
        if current.cost > time_limit {
            continue;
        }
        visited_nodes.insert(current.value);

        let mut any = false;
        for b in dir_road_to_bldgs.get(current.value) {
            any = true;
            bldg_results.insert(*b, current.cost);
        }
        if !any {
            road_results
                .entry(current.value.road)
                .or_insert(current.cost);
        }

        for mvmnt in map.get_movements_for(current.value, constraints) {
            if let Some(cost) =
                vehicle_cost(mvmnt.from, mvmnt, constraints, map.routing_params(), map)
            {
                queue.push(PriorityQueueItem {
                    cost: current.cost + cost,
                    value: mvmnt.to,
                });
            }
        }
    }

    (bldg_results, road_results)
}

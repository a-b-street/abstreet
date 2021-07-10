// TODO Possibly these should be methods on Map.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use petgraph::graphmap::DiGraphMap;

use geom::Duration;

pub use self::walking::{all_walking_costs_from, WalkingOptions};
use crate::pathfind::{fast_paths_to_petgraph, unround, zone_cost, Node, VehiclePathTranslator};
pub use crate::pathfind::{vehicle_cost, WalkingNode};
use crate::{
    BuildingID, DirectedRoadID, IntersectionID, LaneID, Map, PathConstraints, PathRequest,
};

mod walking;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Spot {
    Building(BuildingID),
    Border(IntersectionID),
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
        .values()
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

/// Starting from some initial spot, calculate the cost to all buildings. If a destination isn't
/// reachable, it won't be included in the results. Ignore results greater than the time_limit
/// away.
pub fn all_vehicle_costs_from(
    map: &Map,
    starts: Vec<Spot>,
    time_limit: Duration,
    constraints: PathConstraints,
) -> HashMap<BuildingID, Duration> {
    assert!(constraints != PathConstraints::Pedestrian);
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

    let mut queue: BinaryHeap<Item> = BinaryHeap::new();

    for spot in starts {
        match spot {
            Spot::Building(b_id) => {
                if let Some(start_road) = bldg_to_road.get(&b_id).cloned() {
                    queue.push(Item {
                        cost: Duration::ZERO,
                        node: start_road,
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
                    queue.push(Item {
                        cost: Duration::ZERO,
                        node: map.get_l(l_id).get_directed_parent(),
                    });
                }
            }
        }
    }

    let mut cost_per_node: HashMap<DirectedRoadID, Duration> = HashMap::new();
    while let Some(current) = queue.pop() {
        if cost_per_node.contains_key(&current.node) {
            continue;
        }
        if current.cost > time_limit {
            continue;
        }
        cost_per_node.insert(current.node, current.cost);

        for mvmnt in map.get_movements_for(current.node, constraints) {
            queue.push(Item {
                cost: current.cost
                    + vehicle_cost(mvmnt.from, mvmnt, constraints, map.routing_params(), map)
                    + zone_cost(mvmnt, constraints, map),
                node: mvmnt.to,
            });
        }
    }

    let mut results = HashMap::new();
    for (b, road) in bldg_to_road {
        if let Some(duration) = cost_per_node.get(&road).cloned() {
            results.insert(b, duration);
        }
    }
    results
}

#[derive(PartialEq, Eq)]
struct Item {
    cost: Duration,
    node: DirectedRoadID,
}
impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Item) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Item) -> Ordering {
        // BinaryHeap is a max-heap, so reverse the comparison to get smallest times first.
        let ord = other.cost.cmp(&self.cost);
        if ord != Ordering::Equal {
            return ord;
        }
        self.node.cmp(&other.node)
    }
}

/// Return the cost of a single path, and also a mapping from every directed road to the cost of
/// getting there from the same start. This can be used to understand why an alternative route
/// wasn't chosen.
pub fn debug_vehicle_costs(
    req: PathRequest,
    map: &Map,
) -> Option<(Duration, HashMap<DirectedRoadID, Duration>)> {
    // TODO Support this
    if req.constraints == PathConstraints::Pedestrian {
        return None;
    }

    let cost =
        crate::pathfind::dijkstra::pathfind(req.clone(), map.routing_params(), map)?.get_cost();

    let translator = VehiclePathTranslator::new(map, req.constraints);
    let input_graph = translator.make_input_graph(map.routing_params(), map);
    let graph = fast_paths_to_petgraph(input_graph);

    let start = translator.nodes.get(Node::Road(
        map.get_l(req.start.lane()).get_directed_parent(),
    ));
    let raw_road_costs = petgraph::algo::dijkstra(&graph, start, None, |(_, _, cost)| *cost);
    let mut road_costs = HashMap::new();
    for (node, cost) in raw_road_costs {
        if let Node::Road(dr) = translator.nodes.translate_id(node) {
            road_costs.insert(dr, unround(cost));
        }
    }
    Some((cost, road_costs))
}

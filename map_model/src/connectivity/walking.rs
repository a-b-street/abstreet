use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use geom::{Duration, Speed};

use crate::connectivity::Spot;
use crate::pathfind::{zone_cost, WalkingNode};
use crate::{BuildingID, Lane, LaneType, Map, PathConstraints, Traversable};

#[derive(Clone)]
pub struct WalkingOptions {
    /// If true, allow walking on shoulders.
    pub allow_shoulders: bool,
    pub walking_speed: Speed,
}

impl WalkingOptions {
    pub fn default() -> WalkingOptions {
        WalkingOptions {
            allow_shoulders: true,
            walking_speed: WalkingOptions::default_speed(),
        }
    }

    pub fn common_speeds() -> Vec<(&'static str, Speed)> {
        vec![
            ("3 mph (average for an adult)", Speed::miles_per_hour(3.0)),
            ("1 mph (manual wheelchair)", Speed::miles_per_hour(1.0)),
            ("5 mph (moderate jog)", Speed::miles_per_hour(5.0)),
        ]
    }

    pub fn default_speed() -> Speed {
        WalkingOptions::common_speeds()[0].1
    }
}

#[derive(PartialEq, Eq)]
struct Item {
    cost: Duration,
    node: WalkingNode,
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

/// Starting from some initial buildings, calculate the cost to all others. If a destination isn't
/// reachable, it won't be included in the results. Ignore results greater than the time_limit
/// away.
///
/// If all of the start buildings are on the shoulder of a road and `!opts.allow_shoulders`, then
/// the results will always be empty.
pub fn all_walking_costs_from(
    map: &Map,
    starts: Vec<Spot>,
    time_limit: Duration,
    opts: WalkingOptions,
) -> HashMap<BuildingID, Duration> {
    let mut queue: BinaryHeap<Item> = BinaryHeap::new();

    for spot in starts {
        match spot {
            Spot::Building(b_id) => {
                if opts.allow_shoulders
                    && map.get_l(map.get_b(b_id).sidewalk()).lane_type != LaneType::Shoulder
                {
                    queue.push(Item {
                        cost: Duration::ZERO,
                        node: WalkingNode::closest(map.get_b(b_id).sidewalk_pos, map),
                    });
                }
            }
            Spot::Border(i_id) => {
                let intersection = map.get_i(i_id);
                let incoming_lanes = intersection.incoming_lanes.clone();
                let mut outgoing_lanes = intersection.outgoing_lanes.clone();
                let mut all_lanes = incoming_lanes;
                all_lanes.append(&mut outgoing_lanes);
                let walkable_lanes: Vec<&Lane> = all_lanes
                    .iter()
                    .map(|l_id| map.get_l(l_id.clone()))
                    .filter(|l| l.is_walkable())
                    .collect();
                for lane in walkable_lanes {
                    queue.push(Item {
                        cost: Duration::ZERO,
                        node: WalkingNode::SidewalkEndpoint(
                            lane.get_directed_parent(),
                            lane.src_i == i_id,
                        ),
                    });
                }
            }
        }
    }

    let mut cost_per_node: HashMap<WalkingNode, Duration> = HashMap::new();
    while let Some(current) = queue.pop() {
        if cost_per_node.contains_key(&current.node) {
            continue;
        }
        if current.cost > time_limit {
            continue;
        }
        cost_per_node.insert(current.node, current.cost);

        let (r, is_dst_i) = match current.node {
            WalkingNode::SidewalkEndpoint(r, is_dst_i) => (r, is_dst_i),
            _ => unreachable!(),
        };
        let lane = map.get_l(r.must_get_sidewalk(map));
        // Cross the lane
        if opts.allow_shoulders || lane.lane_type != LaneType::Shoulder {
            queue.push(Item {
                cost: current.cost
                    + lane.length()
                        / Traversable::Lane(lane.id).max_speed_along(
                            Some(opts.walking_speed),
                            PathConstraints::Pedestrian,
                            map,
                        ),
                node: WalkingNode::SidewalkEndpoint(r, !is_dst_i),
            });
        }
        // All turns from the lane
        for turn in map.get_turns_for(lane.id, PathConstraints::Pedestrian) {
            if (turn.id.parent == lane.dst_i) != is_dst_i {
                continue;
            }
            queue.push(Item {
                cost: current.cost
                    + turn.geom.length()
                        / Traversable::Turn(turn.id).max_speed_along(
                            Some(opts.walking_speed),
                            PathConstraints::Pedestrian,
                            map,
                        )
                    + zone_cost(turn.id.to_movement(map), PathConstraints::Pedestrian, map),
                node: WalkingNode::SidewalkEndpoint(
                    map.get_l(turn.id.dst).get_directed_parent(),
                    map.get_l(turn.id.dst).dst_i == turn.id.parent,
                ),
            });
        }
    }

    let mut results = HashMap::new();
    // Assign every building a cost based on which end of the sidewalk it's closest to
    // TODO We could try to get a little more accurate by accounting for the distance from that
    // end of the sidewalk to the building
    for b in map.all_buildings() {
        if let Some(cost) = cost_per_node.get(&WalkingNode::closest(b.sidewalk_pos, map)) {
            let sidewalk_len = map.get_l(b.sidewalk()).length();
            let bldg_dist = b.sidewalk_pos.dist_along();
            let distance_from_closest_node = if sidewalk_len - bldg_dist <= bldg_dist {
                bldg_dist
            } else {
                sidewalk_len - bldg_dist
            };
            let total_cost = *cost + distance_from_closest_node / opts.walking_speed;
            results.insert(b.id, total_cost);
        }
    }

    results
}

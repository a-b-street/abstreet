use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use abstutil::MultiMap;
use geom::{Duration, Speed};

use crate::connectivity::Spot;
use crate::pathfind::{zone_cost, WalkingNode};
use crate::{BuildingID, Lane, LaneType, Map, PathConstraints, PathStep};

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
                queue.push(Item {
                    cost: Duration::ZERO,
                    node: WalkingNode::closest(map.get_b(b_id).sidewalk_pos, map),
                });
            }
            Spot::Border(i_id) => {
                let intersection = map.get_i(i_id);
                let incoming_lanes = intersection.incoming_lanes.clone();
                let mut outgoing_lanes = intersection.outgoing_lanes.clone();
                let mut all_lanes = incoming_lanes;
                all_lanes.append(&mut outgoing_lanes);
                let walkable_lanes: Vec<&Lane> = all_lanes
                    .into_iter()
                    .map(|l_id| map.get_l(l_id))
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
            Spot::DirectedRoad(dr) => {
                // Start from either end
                queue.push(Item {
                    cost: Duration::ZERO,
                    node: WalkingNode::SidewalkEndpoint(dr, false),
                });
                queue.push(Item {
                    cost: Duration::ZERO,
                    node: WalkingNode::SidewalkEndpoint(dr, true),
                });
            }
        }
    }

    if !opts.allow_shoulders {
        let mut shoulder_endpoint = Vec::new();
        for q in &queue {
            if let WalkingNode::SidewalkEndpoint(dir_r, _) = q.node {
                let lanes = &map.get_r(dir_r.id).lanes_ltr;
                for (_, _, lane_type) in lanes {
                    shoulder_endpoint.push(lane_type == &LaneType::Shoulder)
                }
            }
        }
        if shoulder_endpoint.into_iter().all(|x| x) {
            return HashMap::new();
        }
    }

    let mut sidewalk_to_bldgs = MultiMap::new();
    for b in map.all_buildings() {
        sidewalk_to_bldgs.insert(b.sidewalk(), b.id);
    }

    let mut results = HashMap::new();

    let mut visited_nodes = HashSet::new();
    while let Some(current) = queue.pop() {
        if visited_nodes.contains(&current.node) {
            continue;
        }
        if current.cost > time_limit {
            continue;
        }
        visited_nodes.insert(current.node);

        let (r, is_dst_i) = match current.node {
            WalkingNode::SidewalkEndpoint(r, is_dst_i) => (r, is_dst_i),
            _ => unreachable!(),
        };
        let lane = map.get_l(r.must_get_sidewalk(map));
        // Cross the lane
        if opts.allow_shoulders || lane.lane_type != LaneType::Shoulder {
            let sidewalk_len = lane.length();
            let step = if is_dst_i {
                PathStep::ContraflowLane(lane.id)
            } else {
                PathStep::Lane(lane.id)
            };
            let speed =
                step.max_speed_along(Some(opts.walking_speed), PathConstraints::Pedestrian, map);
            let cross_to_node = WalkingNode::SidewalkEndpoint(r, !is_dst_i);

            // We're crossing the sidewalk from one end to the other. If we haven't already found a
            // shorter path to the other end of this sidewalk, then fill out the exact distance to
            // each building. We need to know the direction along the sidewalk we're moving to fill
            // this out properly, so that's why the order of graph nodes visited matters and we're
            // doing this work here.
            if !visited_nodes.contains(&cross_to_node) {
                for b in sidewalk_to_bldgs.get(lane.id) {
                    let bldg_dist_along = map.get_b(*b).sidewalk_pos.dist_along();
                    let dist_to_bldg = if is_dst_i {
                        // Crossing from the end of the sidewalk to the beginning
                        sidewalk_len - bldg_dist_along
                    } else {
                        bldg_dist_along
                    };
                    let bldg_cost = current.cost + dist_to_bldg / speed;
                    if bldg_cost <= time_limit {
                        results.insert(*b, bldg_cost);
                    }
                }

                queue.push(Item {
                    cost: current.cost + sidewalk_len / speed,
                    node: cross_to_node,
                });
            }
        }
        // All turns from the lane
        for turn in map.get_turns_for(lane.id, PathConstraints::Pedestrian) {
            if (turn.id.parent == lane.dst_i) != is_dst_i {
                continue;
            }
            queue.push(Item {
                cost: current.cost
                    + turn.geom.length()
                        / PathStep::Turn(turn.id).max_speed_along(
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

    results
}

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use geom::{Distance, Duration, Speed};

use crate::pathfind::WalkingNode;
use crate::{BuildingID, LaneType, Map, PathConstraints};

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

    fn cost(&self, dist: Distance) -> Duration {
        dist / self.walking_speed
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

/// Starting from one building, calculate the cost to all others. If a destination isn't reachable,
/// it won't be included in the results. Ignore results greater than the time_limit away.
///
/// If the start building is on the shoulder of a road and `!opts.allow_shoulders`, then the
/// results will always be empty.
pub fn all_walking_costs_from(
    map: &Map,
    start: BuildingID,
    time_limit: Duration,
    opts: WalkingOptions,
) -> HashMap<BuildingID, Duration> {
    let start_lane = map.get_l(map.get_b(start).sidewalk_pos.lane());
    if start_lane.lane_type == LaneType::Shoulder && !opts.allow_shoulders {
        return HashMap::new();
    }

    let mut queue: BinaryHeap<Item> = BinaryHeap::new();
    queue.push(Item {
        cost: Duration::ZERO,
        node: WalkingNode::closest(map.get_b(start).sidewalk_pos, map),
    });

    let mut cost_per_node: HashMap<WalkingNode, Duration> = HashMap::new();
    while let Some(current) = queue.pop() {
        if cost_per_node.contains_key(&current.node) {
            continue;
        }
        if current.cost > time_limit {
            continue;
        }
        cost_per_node.insert(current.node, current.cost);

        let (l, is_dst_i) = match current.node {
            WalkingNode::SidewalkEndpoint(l, is_dst_i) => (l, is_dst_i),
            _ => unreachable!(),
        };
        let lane = map.get_l(l);
        // Cross the lane
        if opts.allow_shoulders || lane.lane_type != LaneType::Shoulder {
            queue.push(Item {
                cost: current.cost + opts.cost(lane.length()),
                node: WalkingNode::SidewalkEndpoint(lane.id, !is_dst_i),
            });
        }
        // All turns from the lane
        for turn in map.get_turns_for(lane.id, PathConstraints::Pedestrian) {
            if (turn.id.parent == lane.dst_i) != is_dst_i {
                continue;
            }
            queue.push(Item {
                cost: current.cost + opts.cost(turn.geom.length()),
                node: WalkingNode::SidewalkEndpoint(
                    turn.id.dst,
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

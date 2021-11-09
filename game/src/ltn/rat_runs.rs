use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};

use geom::Duration;
use map_model::{
    connectivity, DirectedRoadID, DrivingSide, IntersectionID, Map, PathConstraints, PathRequest,
    PathV2, RoadID, TurnType,
};

use super::Neighborhood;

pub struct RatRun {
    pub shortcut_path: PathV2,
    /// May be the same as the shortcut
    pub fastest_path: PathV2,
}

/// Ideally this returns every possible path through the neighborhood between two borders. Doesn't
/// work correctly yet.
pub fn find_rat_runs(
    map: &Map,
    neighborhood: &Neighborhood,
    modal_filters: &BTreeSet<RoadID>,
) -> Vec<RatRun> {
    let mut results: Vec<RatRun> = Vec::new();
    for i in &neighborhood.borders {
        let mut started_from: HashSet<DirectedRoadID> = HashSet::new();
        for l in map.get_i(*i).get_outgoing_lanes(map, PathConstraints::Car) {
            let dr = map.get_l(l).get_directed_parent();
            if !started_from.contains(&dr) && neighborhood.orig_perimeter.interior.contains(&dr.id)
            {
                started_from.insert(dr);
                results.extend(find_rat_runs_from(
                    map,
                    dr,
                    &neighborhood.borders,
                    modal_filters,
                ));
            }
        }
    }
    results.sort_by(|a, b| a.time_ratio().partial_cmp(&b.time_ratio()).unwrap());
    results
}

fn find_rat_runs_from(
    map: &Map,
    start: DirectedRoadID,
    borders: &BTreeSet<IntersectionID>,
    modal_filters: &BTreeSet<RoadID>,
) -> Vec<RatRun> {
    // If there's a filter where we're starting, we can't go anywhere
    if modal_filters.contains(&start.id) {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut back_refs = HashMap::new();
    let mut queue: BinaryHeap<Item> = BinaryHeap::new();
    queue.push(Item {
        node: start,
        cost: Duration::ZERO,
    });
    let mut visited = HashSet::new();

    while let Some(current) = queue.pop() {
        if visited.contains(&current.node) {
            continue;
        }
        visited.insert(current.node);

        // If we found a border, then stitch together the path
        let dst_i = current.node.dst_i(map);
        if borders.contains(&dst_i) {
            let mut at = current.node;
            let mut path = vec![at];
            while let Some(prev) = back_refs.get(&at).cloned() {
                path.push(prev);
                at = prev;
            }
            path.push(start);
            path.reverse();
            results.push(RatRun::new(map, path, current.cost));
            // TODO Keep searching for more, but infinite loop currently
            return results;
        }

        for mvmnt in map.get_movements_for(current.node, PathConstraints::Car) {
            // Can't cross filters
            if modal_filters.contains(&mvmnt.to.id) {
                continue;
            }

            queue.push(Item {
                cost: current.cost
                    + connectivity::vehicle_cost(
                        mvmnt.from,
                        mvmnt,
                        PathConstraints::Car,
                        map.routing_params(),
                        map,
                    )
                    + connectivity::zone_cost(mvmnt, PathConstraints::Car, map),
                node: mvmnt.to,
            });
            back_refs.insert(mvmnt.to, mvmnt.from);
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

impl RatRun {
    fn new(map: &Map, mut path: Vec<DirectedRoadID>, cost: Duration) -> RatRun {
        let entry = cheap_entry(map, path[0]);
        let exit = cheap_exit(map, *path.last().unwrap());
        path.insert(0, entry);
        path.push(exit);
        // TODO Adjust the cost!

        let req =
            PathRequest::between_directed_roads(map, entry, exit, PathConstraints::Car).unwrap();
        let shortcut_path = PathV2::from_roads(
            path,
            req.clone(),
            cost,
            // TODO We're assuming there are no uber turns. Seems unlikely in the interior of a
            // neighborhood!
            Vec::new(),
            map,
        );
        let fastest_path = map.pathfind_v2(req).unwrap();
        // TODO If the path matches up, double check the cost does too, since we may calculate it
        // differently...
        RatRun {
            shortcut_path,
            fastest_path,
        }
    }

    /// The ratio of the shortcut's time to the fastest path's time. Smaller values mean the
    /// shortcut is more desirable.
    pub fn time_ratio(&self) -> f64 {
        // TODO Not sure why yet, just avoid crashing
        if self.fastest_path.get_cost() == Duration::ZERO {
            return 1.0;
        }

        self.shortcut_path.get_cost() / self.fastest_path.get_cost()
    }
}

/// Find a road that leads into the neighborhood at a particular intersection.
fn cheap_entry(map: &Map, to: DirectedRoadID) -> DirectedRoadID {
    let cheap_turn_type = if map.get_config().driving_side == DrivingSide::Right {
        TurnType::Right
    } else {
        TurnType::Left
    };
    let cheap_turn = map
        .get_i(to.src_i(map))
        .turns
        .iter()
        .filter(|t| t.id.dst.road == to.id)
        .min_by_key(|t| {
            if t.turn_type == cheap_turn_type {
                0
            } else if t.turn_type == TurnType::Straight {
                1
            } else {
                2
            }
        })
        .unwrap();
    // TODO We're assuming this source road also leads somewhere else on the perimeter
    map.get_l(cheap_turn.id.src).get_directed_parent()
}

/// Find a road that leads out of the neighborhood at a particular intersection.
fn cheap_exit(map: &Map, from: DirectedRoadID) -> DirectedRoadID {
    let cheap_turn_type = if map.get_config().driving_side == DrivingSide::Right {
        TurnType::Right
    } else {
        TurnType::Left
    };
    let cheap_turn = map
        .get_i(from.dst_i(map))
        .turns
        .iter()
        .filter(|t| t.id.src.road == from.id)
        .min_by_key(|t| {
            if t.turn_type == cheap_turn_type {
                0
            } else if t.turn_type == TurnType::Straight {
                1
            } else {
                2
            }
        })
        .unwrap();
    map.get_l(cheap_turn.id.dst).get_directed_parent()
}

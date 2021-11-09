use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};

use geom::Duration;
use map_model::{
    connectivity, DirectedRoadID, DrivingSide, IntersectionID, Map, MovementID, PathConstraints,
    PathRequest, PathV2, TurnType,
};

use super::{ModalFilters, Neighborhood};

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
    modal_filters: &ModalFilters,
) -> Vec<RatRun> {
    let mut results: Vec<RatRun> = Vec::new();
    for i in &neighborhood.borders {
        let mut started_from: HashSet<DirectedRoadID> = HashSet::new();
        for l in map.get_i(*i).get_outgoing_lanes(map, PathConstraints::Car) {
            let dr = map.get_l(l).get_directed_parent();
            if !started_from.contains(&dr)
                && neighborhood.orig_perimeter.interior.contains(&dr.road)
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
    modal_filters: &ModalFilters,
) -> Vec<RatRun> {
    // If there's a filter where we're starting, we can't go anywhere
    if modal_filters.roads.contains_key(&start.road) {
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
            // Keep searching for more
            continue;
        }

        for mvmnt in map.get_movements_for(current.node, PathConstraints::Car) {
            // Can't cross filters
            if modal_filters.roads.contains_key(&mvmnt.to.road) {
                continue;
            }
            // If we've already visited the destination, don't add it again. We don't want to
            // update back_refs -- because this must be a higher-cost path to a place we've already
            // visited.
            if visited.contains(&mvmnt.to) {
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
                    ),
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
    fn new(map: &Map, mut path: Vec<DirectedRoadID>, mut cost: Duration) -> RatRun {
        // The rat run starts and ends at a road just inside the neighborhood. To "motivate" using
        // the shortcut, find an entry and exit road just outside the neighborhood to calculate a
        // fastest path.
        let entry = cheap_entry(map, path[0]);
        let exit = cheap_exit(map, *path.last().unwrap());
        path.insert(0, entry.from);
        path.push(exit.to);

        // Adjust the cost for the new roads
        // TODO Or just make a PathV2 method to do this?
        cost += connectivity::vehicle_cost(
            entry.from,
            entry,
            PathConstraints::Car,
            map.routing_params(),
            map,
        );
        cost += connectivity::vehicle_cost(
            // TODO This is an abuse of vehicle_cost! It should just take the MovementID and always
            // use from... and something else should add the cost of the final road
            exit.to,
            exit,
            PathConstraints::Car,
            map.routing_params(),
            map,
        );

        let req =
            PathRequest::between_directed_roads(map, entry.from, exit.to, PathConstraints::Car)
                .unwrap();
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

/// Find a movement that leads into the neighborhood at the first road in a rat-run
fn cheap_entry(map: &Map, to: DirectedRoadID) -> MovementID {
    let cheap_turn_type = if map.get_config().driving_side == DrivingSide::Right {
        TurnType::Right
    } else {
        TurnType::Left
    };
    map.get_i(to.src_i(map))
        .movements
        .values()
        .filter(|mvmnt| mvmnt.id.to == to)
        .min_by_key(|mvmnt| {
            if mvmnt.turn_type == cheap_turn_type {
                0
            } else if mvmnt.turn_type == TurnType::Straight {
                1
            } else {
                2
            }
        })
        .unwrap()
        .id
}

/// Find a movement that leads out of the neighborhood at the last road in a rat-run
fn cheap_exit(map: &Map, from: DirectedRoadID) -> MovementID {
    let cheap_turn_type = if map.get_config().driving_side == DrivingSide::Right {
        TurnType::Right
    } else {
        TurnType::Left
    };
    map.get_i(from.dst_i(map))
        .movements
        .values()
        .filter(|mvmnt| mvmnt.id.from == from)
        .min_by_key(|mvmnt| {
            if mvmnt.turn_type == cheap_turn_type {
                0
            } else if mvmnt.turn_type == TurnType::Straight {
                1
            } else {
                2
            }
        })
        .unwrap()
        .id
}

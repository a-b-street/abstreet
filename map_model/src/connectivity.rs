use crate::{IntersectionID, LaneID, Map, PathConstraints};
use abstutil::Timer;
use geom::Duration;
use petgraph::graphmap::DiGraphMap;
use std::collections::{HashMap, HashSet, VecDeque};

// SCC = strongly connected component

// Returns (relevant lanes in main component, disconnected relevant lanes)
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

// Returns list of (driving lane, redirect here instead for parking)
//
// It's a bit weird to never attempt parking on roads not part of the largest SCC of the graph.
// This is acceptable, because there shouldn't be too many roads outside of that SCC anyway.
pub fn redirect_parking_blackholes(map: &Map, timer: &mut Timer) -> Vec<(LaneID, LaneID)> {
    let (largest_group, disconnected) = find_scc(map, PathConstraints::Car);

    let mut redirects = Vec::new();
    timer.start_iter("find parking blackhole redirects", disconnected.len());
    for l in disconnected {
        timer.next();

        // Search forwards and backwards for the nearest driving lane belonging to largest_group.
        if let Some(redirect) = bidi_flood(map, l, &largest_group) {
            redirects.push((l, redirect));
        } else {
            // TODO Make this an error after dealing with places like Austin without much parking
            // in the first place.
            timer.warn(format!(
                "{} is a parking blackhole with no reasonable redirect!",
                l
            ));
        }
    }
    timer.note(format!(
        "{} driving lanes are parking blackholes",
        redirects.len()
    ));
    redirects
}

fn bidi_flood(map: &Map, start: LaneID, largest_group: &HashSet<LaneID>) -> Option<LaneID> {
    let mut queue = VecDeque::new();
    queue.push_back(start);
    let mut visisted = HashSet::new();

    while !queue.is_empty() {
        let current = queue.pop_front().unwrap();
        if visisted.contains(&current) {
            continue;
        }
        visisted.insert(current);
        if largest_group.contains(&current) {
            return Some(current);
        }
        for turn in map.get_turns_to_lane(current) {
            queue.push_back(turn.id.src);
        }
        for turn in map.get_turns_for(current, PathConstraints::Car) {
            queue.push_back(turn.id.dst);
        }
    }
    None
}

// TODO Different cost function
// TODO Cost per building
pub fn all_costs_from(map: &Map, start: IntersectionID) -> HashMap<IntersectionID, Duration> {
    let mut graph = DiGraphMap::new();
    for r in map.all_roads() {
        graph.add_edge(r.src_i, r.dst_i, r.id);
    }

    petgraph::algo::dijkstra(&graph, start, None, |(_, _, r)| {
        let r = map.get_r(*r);
        r.center_pts.length() / r.speed_limit
    })
}

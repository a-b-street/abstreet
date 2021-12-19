use std::collections::HashSet;

use abstutil::{Counter, Timer};
use map_model::{
    DirectedRoadID, IntersectionID, LaneID, Map, Path, PathConstraints, PathRequest, PathStep,
    Position, RoadID,
};

use super::{ModalFilters, Neighborhood};

pub struct RatRuns {
    pub paths: Vec<Path>,
    pub count_per_road: Counter<RoadID>,
    pub count_per_intersection: Counter<IntersectionID>,
}

pub fn find_rat_runs(
    map: &Map,
    neighborhood: &Neighborhood,
    modal_filters: &ModalFilters,
    timer: &mut Timer,
) -> RatRuns {
    let entrances = find_entrances(map, neighborhood);
    let exits = find_exits(map, neighborhood);

    // Look for all possible paths from an entrance to an exit, only if they connect to different
    // major roads.
    let mut requests = Vec::new();
    for entrance in &entrances {
        for exit in &exits {
            if entrance.major_road_name != exit.major_road_name {
                requests.push(PathRequest::vehicle(
                    Position::start(entrance.lane),
                    Position::end(exit.lane, map),
                    PathConstraints::Car,
                ));
            }
        }
    }

    let mut params = map.routing_params().clone();
    modal_filters.update_routing_params(&mut params);
    let cache_custom = true;
    let mut paths: Vec<Path> = timer
        .parallelize(
            "calculate paths between entrances and exits",
            requests,
            |req| map.pathfind_with_params(req, &params, cache_custom),
        )
        .into_iter()
        .flatten()
        .collect();

    // update_routing_params heavily penalizes crossing modal filters, but it doesn't prevent it
    // completely! So strip out paths that were forced to cross a filter.
    paths.retain(|path| {
        !path.get_steps().iter().any(|step| match step {
            PathStep::Lane(l) => modal_filters.roads.contains_key(&l.road),
            PathStep::Turn(t) => !modal_filters.allows_turn(*t),
            // Car paths don't make contraflow movements
            _ => unreachable!(),
        })
    });

    // Some paths wind up partly using perimeter roads (or even things outside the neighborhood
    // entirely). Sort by "worse" paths that spend more time inside.
    paths.sort_by_key(|path| {
        let mut roads_inside = 0;
        let mut roads_outside = 0;
        for step in path.get_steps() {
            if let PathStep::Lane(l) = step {
                if neighborhood.orig_perimeter.interior.contains(&l.road) {
                    roads_inside += 1;
                } else {
                    roads_outside += 1;
                }
            }
        }
        let pct = (roads_outside as f64) / (roads_outside + roads_inside) as f64;
        // f64 isn't Ord, just approximate by 1/10th of a percent
        (pct * 1000.0) as usize
    });

    // How many rat-runs pass through each street?
    let mut count_per_road = Counter::new();
    let mut count_per_intersection = Counter::new();
    for path in &paths {
        for step in path.get_steps() {
            match step {
                PathStep::Lane(l) => {
                    if neighborhood.orig_perimeter.interior.contains(&l.road) {
                        count_per_road.inc(l.road);
                    }
                }
                PathStep::Turn(t) => {
                    if neighborhood.interior_intersections.contains(&t.parent) {
                        count_per_intersection.inc(t.parent);
                    }
                }
                // Car paths don't make contraflow movements
                _ => unreachable!(),
            }
        }
    }

    RatRuns {
        paths,
        count_per_road,
        count_per_intersection,
    }
}

struct EntryExit {
    // TODO Really this is a DirectedRoadID, but since pathfinding later needs to know lanes, just
    // use this
    lane: LaneID,
    major_road_name: String,
}

fn find_entrances(map: &Map, neighborhood: &Neighborhood) -> Vec<EntryExit> {
    let mut entrances = Vec::new();
    for i in &neighborhood.borders {
        if let Some(major_road_name) = find_major_road_name(map, neighborhood, *i) {
            let mut seen: HashSet<DirectedRoadID> = HashSet::new();
            for l in map.get_i(*i).get_outgoing_lanes(map, PathConstraints::Car) {
                let dr = map.get_l(l).get_directed_parent();
                if !seen.contains(&dr) && neighborhood.orig_perimeter.interior.contains(&dr.road) {
                    entrances.push(EntryExit {
                        lane: l,
                        major_road_name: major_road_name.clone(),
                    });
                    seen.insert(dr);
                }
            }
        }
    }
    entrances
}

fn find_exits(map: &Map, neighborhood: &Neighborhood) -> Vec<EntryExit> {
    let mut exits = Vec::new();
    for i in &neighborhood.borders {
        if let Some(major_road_name) = find_major_road_name(map, neighborhood, *i) {
            let mut seen: HashSet<DirectedRoadID> = HashSet::new();
            for l in map.get_i(*i).get_incoming_lanes(map, PathConstraints::Car) {
                let dr = map.get_l(l).get_directed_parent();
                if !seen.contains(&dr) && neighborhood.orig_perimeter.interior.contains(&dr.road) {
                    exits.push(EntryExit {
                        lane: l,
                        major_road_name: major_road_name.clone(),
                    });
                    seen.insert(dr);
                }
            }
        }
    }
    exits
}

fn find_major_road_name(
    map: &Map,
    neighborhood: &Neighborhood,
    i: IntersectionID,
) -> Option<String> {
    let mut names = Vec::new();
    for r in &map.get_i(i).roads {
        if neighborhood.perimeter.contains(r) {
            names.push(map.get_r(*r).get_name(None));
        }
    }
    names.sort();
    names.dedup();
    // TODO If the major road changes names or we found a corner, bail out
    if names.len() == 1 {
        names.pop()
    } else {
        None
    }
}

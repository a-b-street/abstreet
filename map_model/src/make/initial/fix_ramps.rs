use crate::make::initial::{geometry, InitialMap};
use crate::raw_data::{StableIntersectionID, StableRoadID};
use abstutil::Timer;
use std::collections::{BTreeSet, HashSet};

pub fn fix_ramps(m: &mut InitialMap, timer: &mut Timer) {
    if m.roads.len() > 15_000 {
        error!("Skipping fix_ramps because map is too big! TODO: Optimize me!");
        return;
    }

    // Look for road center lines that hit an intersection polygon that isn't one of their
    // endpoints.
    timer.start_iter(
        "look for roads crossing intersections in strange ways",
        m.roads.len(),
    );
    let mut fixme: Vec<(StableRoadID, StableIntersectionID)> = Vec::new();
    for r in m.roads.values() {
        timer.next();
        // TODO Prune search.
        for i in m.intersections.values() {
            if r.src_i == i.id || r.dst_i == i.id {
                continue;
            }
            if !r.trimmed_center_pts.crosses_polygon(&i.polygon) {
                continue;
            }

            // TODO Avoid some false positives by seeing if this road is "close" to the
            // intersection it crosses. This probably needs more tuning. It avoids expected
            // tunnel/bridge crossings.
            if !floodfill(m, i.id, 5).contains(&r.id) {
                continue;
            }

            // TODO Still seeing false positives due to lack of short road merging.

            fixme.push((r.id, i.id));
        }
    }

    for (r, i) in fixme {
        if fix_ramp(m, r, i) {
            info!("Fixed ramp {} crossing {}", r, i);
        } else {
            info!("{} crosses {} strangely, but didn't change anything", r, i);
        }
    }
}

fn floodfill(m: &InitialMap, start: StableIntersectionID, steps: usize) -> HashSet<StableRoadID> {
    let mut seen: HashSet<StableRoadID> = HashSet::new();
    let mut queue: Vec<(StableRoadID, usize)> = m.intersections[&start]
        .roads
        .iter()
        .map(|r| (*r, 1))
        .collect();
    while !queue.is_empty() {
        let (r, count) = queue.pop().unwrap();
        if seen.contains(&r) {
            continue;
        }
        seen.insert(r);
        if count < steps {
            for next in m.intersections[&m.roads[&r].src_i]
                .roads
                .iter()
                .chain(m.intersections[&m.roads[&r].dst_i].roads.iter())
            {
                queue.push((*next, count + 1));
            }
        }
    }
    seen
}

fn fix_ramp(m: &mut InitialMap, ramp: StableRoadID, new_src: StableIntersectionID) -> bool {
    // Trace backwards...
    let mut delete_roads: Vec<StableRoadID> = Vec::new();
    let mut delete_intersections: Vec<StableIntersectionID> = Vec::new();

    let last_normal_intersection = {
        let mut current_road = ramp;
        loop {
            let src_i = &m.intersections[&m.roads[&current_road].src_i];
            if let Some(other_road) = get_one_other(&src_i.roads, current_road) {
                delete_intersections.push(src_i.id);
                current_road = other_road;
                delete_roads.push(current_road);
            } else {
                break src_i.id;
            }
        }
    };

    if let Some(last_road) = delete_roads.last() {
        let mut i = m.intersections.get_mut(&last_normal_intersection).unwrap();
        i.roads.remove(&last_road);
        i.polygon = geometry::intersection_polygon(i, &mut m.roads);
    } else {
        // TODO Not really sure why, but when there's not a road in between, don't apply the fix.
        return false;
    }
    for r in delete_roads {
        m.roads.remove(&r);
    }
    for i in delete_intersections {
        m.intersections.remove(&i);
    }

    {
        m.roads.get_mut(&ramp).unwrap().src_i = new_src;
        let mut i = m.intersections.get_mut(&new_src).unwrap();
        i.roads.insert(ramp);
        i.polygon = geometry::intersection_polygon(i, &mut m.roads);
    }
    true
}

fn get_one_other<X: PartialEq + Clone>(set: &BTreeSet<X>, item: X) -> Option<X> {
    if set.len() != 2 {
        return None;
    }
    let items: Vec<X> = set.iter().cloned().collect();
    if items[0] == item {
        return Some(items[1].clone());
    }
    Some(items[0].clone())
}

use crate::make::initial::{geometry, InitialMap};
use crate::raw_data::{StableIntersectionID, StableRoadID};
use aabb_quadtree::QuadTree;
use abstutil::Timer;
use geom::Bounds;
use std::collections::{BTreeSet, HashSet};

pub fn fix_ramps(m: &mut InitialMap, timer: &mut Timer) {
    if m.name == "small_seattle" {
        timer.warn(
            "fix_ramps still disabled for small_seattle; crosses_polygon seeing strange results"
                .to_string(),
        );
        return;
    }

    let mut quadtree = QuadTree::default(m.bounds.as_bbox());
    for i in m.intersections.values() {
        quadtree.insert_with_box(i.id, Bounds::from(&i.polygon).as_bbox());
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
        for &(id, _, _) in &quadtree.query(r.trimmed_center_pts.get_bounds().as_bbox()) {
            let i = &m.intersections[&id];
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
        if fix_ramp(m, r, i, timer) {
            timer.note(format!("Fixed ramp {} crossing {}", r, i));
        } else {
            timer.note(format!(
                "{} crosses {} strangely, but didn't change anything",
                r, i
            ));
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

fn fix_ramp(
    m: &mut InitialMap,
    ramp: StableRoadID,
    new_src: StableIntersectionID,
    timer: &mut Timer,
) -> bool {
    // Trace backwards...
    let mut delete_roads: Vec<StableRoadID> = Vec::new();
    let mut delete_intersections: Vec<StableIntersectionID> = Vec::new();

    let last_normal_intersection = {
        let mut current_road = ramp;
        let mut counter = 0;
        loop {
            let src_i = &m.intersections[&m.roads[&current_road].src_i];
            if let Some(other_road) = get_one_other(&src_i.roads, current_road) {
                delete_intersections.push(src_i.id);
                current_road = other_road;
                delete_roads.push(current_road);
            } else {
                break src_i.id;
            }

            counter += 1;
            if counter > 10 {
                timer.warn(format!(
                    "Couldn't find last normal intersection from ramp {}",
                    ramp
                ));
                return false;
            }
        }
    };

    if let Some(last_road) = delete_roads.last() {
        let mut i = m.intersections.get_mut(&last_normal_intersection).unwrap();
        i.roads.remove(&last_road);
        i.polygon = geometry::intersection_polygon(i, &mut m.roads, timer);
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
        i.polygon = geometry::intersection_polygon(i, &mut m.roads, timer);
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

use crate::raw::{OriginalIntersection, OriginalRoad, RawMap};
use abstutil::{retain_btreemap, MultiMap, Timer};
use std::collections::BTreeSet;

pub fn remove_disconnected_roads(map: &mut RawMap, timer: &mut Timer) {
    timer.start("removing disconnected roads");
    // This is a simple floodfill, not Tarjan's. Assumes all roads bidirectional.
    // All the usizes are indices into the original list of roads

    let mut next_roads: MultiMap<OriginalIntersection, OriginalRoad> = MultiMap::new();
    for id in map.roads.keys() {
        next_roads.insert(id.i1, *id);
        next_roads.insert(id.i2, *id);
    }

    let mut partitions: Vec<Vec<OriginalRoad>> = Vec::new();
    let mut unvisited_roads: BTreeSet<OriginalRoad> = map.roads.keys().cloned().collect();

    while !unvisited_roads.is_empty() {
        let mut queue_roads: Vec<OriginalRoad> = vec![*unvisited_roads.iter().next().unwrap()];
        let mut current_partition: Vec<OriginalRoad> = Vec::new();
        while !queue_roads.is_empty() {
            let current = queue_roads.pop().unwrap();
            if !unvisited_roads.contains(&current) {
                continue;
            }
            unvisited_roads.remove(&current);
            current_partition.push(current);

            for other_r in next_roads.get(current.i1).iter() {
                queue_roads.push(*other_r);
            }
            for other_r in next_roads.get(current.i2).iter() {
                queue_roads.push(*other_r);
            }
        }
        partitions.push(current_partition);
    }

    partitions.sort_by_key(|roads| roads.len());
    partitions.reverse();
    println!("Main partition has {} roads", partitions[0].len());
    for p in partitions.iter().skip(1) {
        println!("Removing disconnected partition with {} roads", p.len());
        for id in p {
            map.roads.remove(id).unwrap();
            next_roads.remove(id.i1, *id);
            next_roads.remove(id.i2, *id);
        }
    }

    // Also remove cul-de-sacs here. TODO Support them properly, but for now, they mess up parking
    // hint matching (loop PolyLine) and pathfinding later.
    retain_btreemap(&mut map.roads, |id, _| id.i1 != id.i2);

    // Remove intersections without any roads
    retain_btreemap(&mut map.intersections, |id, _| {
        !next_roads.get(*id).is_empty()
    });
    timer.stop("removing disconnected roads");
}

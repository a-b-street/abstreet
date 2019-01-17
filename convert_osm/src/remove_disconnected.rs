use abstutil::{retain_btreemap, MultiMap, Timer};
use map_model::raw_data;
use std::collections::HashSet;

pub fn remove_disconnected_roads(map: &mut raw_data::Map, timer: &mut Timer) {
    timer.start("removing disconnected roads");
    // This is a simple floodfill, not Tarjan's. Assumes all roads bidirectional.
    // All the usizes are indices into the original list of roads

    let mut next_roads: MultiMap<raw_data::StableIntersectionID, raw_data::StableRoadID> =
        MultiMap::new();
    for (id, r) in &map.roads {
        next_roads.insert(r.i1, *id);
        next_roads.insert(r.i2, *id);
    }

    let mut partitions: Vec<Vec<raw_data::StableRoadID>> = Vec::new();
    let mut unvisited_roads: HashSet<raw_data::StableRoadID> = map.roads.keys().cloned().collect();

    while !unvisited_roads.is_empty() {
        let mut queue_roads: Vec<raw_data::StableRoadID> =
            vec![*unvisited_roads.iter().next().unwrap()];
        let mut current_partition: Vec<raw_data::StableRoadID> = Vec::new();
        while !queue_roads.is_empty() {
            let current = queue_roads.pop().unwrap();
            if !unvisited_roads.contains(&current) {
                continue;
            }
            unvisited_roads.remove(&current);
            current_partition.push(current);

            let current_r = &map.roads[&current];
            for other_r in next_roads.get(current_r.i1).iter() {
                queue_roads.push(*other_r);
            }
            for other_r in next_roads.get(current_r.i2).iter() {
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
            let r = map.roads.remove(id).unwrap();
            next_roads.remove(r.i1, *id);
            next_roads.remove(r.i2, *id);
        }
    }

    // Remove intersections without any roads
    retain_btreemap(
        &mut map.intersections,
        |id, _| !next_roads.get(*id).is_empty(),
    );
    timer.stop("removing disconnected roads");
}

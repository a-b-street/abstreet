use abstutil::MultiMap;
use geom::HashablePt2D;
use map_model::raw_data;
use std::collections::HashSet;

pub fn remove_disconnected_roads(map: &mut raw_data::Map) {
    println!("finding disconnected chunks of road");
    // This is a simple floodfill, not Tarjan's. Assumes all roads bidirectional.
    // All the usizes are indices into the original list of roads

    let mut next_roads: MultiMap<HashablePt2D, usize> = MultiMap::new();
    for (idx, r) in map.roads.iter().enumerate() {
        next_roads.insert(r.first_pt(), idx);
        next_roads.insert(r.last_pt(), idx);
    }

    let mut partitions: Vec<Vec<usize>> = Vec::new();
    let mut unvisited_roads: HashSet<usize> = HashSet::new();
    for i in 0..map.roads.len() {
        unvisited_roads.insert(i);
    }

    while !unvisited_roads.is_empty() {
        let mut queue_roads: Vec<usize> = vec![*unvisited_roads.iter().next().unwrap()];
        let mut current_partition: Vec<usize> = Vec::new();
        while !queue_roads.is_empty() {
            let current = queue_roads.pop().unwrap();
            if !unvisited_roads.contains(&current) {
                continue;
            }
            unvisited_roads.remove(&current);
            current_partition.push(current);

            let current_r = &map.roads[current];
            for other_r in next_roads.get(current_r.first_pt()).iter() {
                queue_roads.push(*other_r);
            }
            for other_r in next_roads.get(current_r.last_pt()).iter() {
                queue_roads.push(*other_r);
            }
        }
        partitions.push(current_partition);
    }

    partitions.sort_by_key(|roads| roads.len());
    partitions.reverse();
    println!("Main partition has {} roads", partitions[0].len());
    let mut remove_roads = HashSet::new();
    for p in partitions.iter().skip(1) {
        println!("Removing disconnected partition with {} roads", p.len());
        for idx in p {
            remove_roads.insert(idx);
        }
    }
    let mut roads: Vec<raw_data::Road> = Vec::new();
    for (idx, r) in map.roads.iter().enumerate() {
        if remove_roads.contains(&idx) {
            next_roads.remove(r.first_pt(), idx);
            next_roads.remove(r.last_pt(), idx);
        } else {
            roads.push(r.clone());
        }
    }
    map.roads = roads;

    // Remove intersections without any roads
    map.intersections
        .retain(|i| !next_roads.get(i.point.to_hashable()).is_empty());
}

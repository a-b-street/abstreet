use abstutil::{MultiMap, Timer};
use geom::HashablePt2D;
use map_model::raw_data;
use std::collections::HashSet;

pub fn remove_disconnected_roads(map: &mut raw_data::Map, timer: &mut Timer) {
    timer.start("removing disconnected roads");
    // This is a simple floodfill, not Tarjan's. Assumes all roads bidirectional.
    // All the usizes are indices into the original list of roads

    let mut next_roads: MultiMap<HashablePt2D, raw_data::StableRoadID> = MultiMap::new();
    for (id, r) in &map.roads {
        next_roads.insert(r.first_pt().to_hashable(), *id);
        next_roads.insert(r.last_pt().to_hashable(), *id);
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
            for other_r in next_roads.get(current_r.first_pt().to_hashable()).iter() {
                queue_roads.push(*other_r);
            }
            for other_r in next_roads.get(current_r.last_pt().to_hashable()).iter() {
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
            next_roads.remove(r.first_pt().to_hashable(), *id);
            next_roads.remove(r.last_pt().to_hashable(), *id);
        }
    }

    // Remove intersections without any roads
    // TODO retain for BTreeMap, please!
    let remove_intersections: Vec<raw_data::StableIntersectionID> = map
        .intersections
        .iter()
        .filter_map(|(id, i)| {
            if next_roads.get(i.point.to_hashable()).is_empty() {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    for id in remove_intersections {
        map.intersections.remove(&id);
    }
    timer.stop("removing disconnected roads");
}

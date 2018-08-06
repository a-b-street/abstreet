use {Parcel, ParcelID, geometry};
use std::collections::BTreeSet;
use abstutil::MultiMap;

pub fn group_parcels(parcels: &mut Vec<Parcel>) {
    // First compute which parcels intersect
    let mut adjacency: MultiMap<ParcelID, ParcelID> = MultiMap::new();
    // TODO could use quadtree to prune
    println!("Precomputing adjacency between {} parcels...", parcels.len());
    let mut adj_counter = 0;
    for p1 in parcels.iter() {
        for p2 in parcels.iter() {
            if p1.id < p2.id && geometry::polygons_intersect(&p1.points, &p2.points) {
                // TODO could do something more clever later to avoid double memory
                adjacency.insert(p1.id, p2.id);
                adjacency.insert(p2.id, p1.id);
                adj_counter += 1;
            }
        }
    }
    println!("{} adjacencies, now doing floodfilling to group them", adj_counter);

    // Union-find might also be good inspiration.
    fn floodfill(from: ParcelID, adj: &MultiMap<ParcelID, ParcelID>) -> BTreeSet<ParcelID> {
        let mut visited: BTreeSet<ParcelID> = BTreeSet::new();
        let mut queue: Vec<ParcelID> = vec![from];
        while !queue.is_empty() {
            let current = queue.pop().unwrap();
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current);
            for next in adj.get(current).iter() {
                queue.push(*next);
            }
        }
        visited
    }

    let mut block_per_parcel: Vec<Option<usize>> = Vec::new();
    for _ in parcels.iter() {
        block_per_parcel.push(None);
    }

    let mut block_counter = 0;
    for base_p in parcels.iter() {
        // A previous iteration might have filled it out
        if block_per_parcel[base_p.id.0].is_some() {
            continue;
        }

        let new_block = Some(block_counter);
        block_counter += 1;
        for p in floodfill(base_p.id, &adjacency).iter() {
            assert!(!block_per_parcel[p.0].is_some());
            block_per_parcel[p.0] = new_block;
        }
    }
    println!("{} parcels grouped into {} blocks", parcels.len(), block_counter);

    for (idx, block) in block_per_parcel.iter().enumerate() {
        parcels[idx].block = block.unwrap();
    }
}

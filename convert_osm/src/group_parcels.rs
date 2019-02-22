use aabb_quadtree::geom::Rect;
use aabb_quadtree::QuadTree;
use abstutil::MultiMap;
use geo;
use geom::{GPSBounds, LonLat};
use map_model::raw_data;
use std::collections::BTreeSet;

// Slight cheat
type ParcelIdx = usize;

pub fn group_parcels(gps_bounds: &GPSBounds, parcels: &mut Vec<raw_data::Parcel>) {
    // Make a quadtree to quickly prune intersections between parcels
    let mut quadtree = QuadTree::default(gps_bounds.as_bbox());
    for (idx, p) in parcels.iter().enumerate() {
        quadtree.insert_with_box(idx, get_bbox(&p.points));
    }

    // First compute which parcels intersect
    let mut adjacency: MultiMap<ParcelIdx, ParcelIdx> = MultiMap::new();
    // TODO could use quadtree to prune
    println!(
        "Precomputing adjacency between {} parcels...",
        parcels.len()
    );
    let mut adj_counter = 0;
    for p1_idx in 0..parcels.len() {
        for &(p2_idx, _, _) in &quadtree.query(get_bbox(&parcels[p1_idx].points)) {
            if p1_idx != *p2_idx
                && polygons_intersect(&parcels[p1_idx].points, &parcels[*p2_idx].points)
            {
                // TODO could do something more clever later to avoid double memory
                adjacency.insert(p1_idx, *p2_idx);
                adjacency.insert(*p2_idx, p1_idx);
                adj_counter += 1;
            }
        }
    }
    println!(
        "{} adjacencies, now doing floodfilling to group them",
        adj_counter
    );

    // Union-find might also be good inspiration.
    fn floodfill(from: ParcelIdx, adj: &MultiMap<ParcelIdx, ParcelIdx>) -> BTreeSet<ParcelIdx> {
        let mut visited: BTreeSet<ParcelIdx> = BTreeSet::new();
        let mut queue: Vec<ParcelIdx> = vec![from];
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
    for base_idx in 0..parcels.len() {
        // A previous iteration might have filled it out
        if block_per_parcel[base_idx].is_some() {
            continue;
        }

        let new_block = Some(block_counter);
        block_counter += 1;
        for idx in floodfill(base_idx, &adjacency).iter() {
            assert!(!block_per_parcel[*idx].is_some());
            block_per_parcel[*idx] = new_block;
        }
    }
    println!(
        "{} parcels grouped into {} blocks",
        parcels.len(),
        block_counter
    );

    for (idx, block) in block_per_parcel.iter().enumerate() {
        parcels[idx].block = block.unwrap();
    }
}

fn polygons_intersect(pts1: &Vec<LonLat>, pts2: &Vec<LonLat>) -> bool {
    use geo::prelude::Intersects;

    let poly1 = geo::Polygon::new(
        pts1.iter()
            .map(|pt| geo::Point::new(pt.longitude, pt.latitude))
            .collect(),
        Vec::new(),
    );
    let poly2 = geo::Polygon::new(
        pts2.iter()
            .map(|pt| geo::Point::new(pt.longitude, pt.latitude))
            .collect(),
        Vec::new(),
    );
    poly1.intersects(&poly2)
}

fn get_bbox(pts: &Vec<LonLat>) -> Rect {
    GPSBounds::from(pts).as_bbox()
}

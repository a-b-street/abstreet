use std::collections::{BTreeSet, HashSet};

use geom::Polygon;
use map_model::osm::RoadRank;
use map_model::{Map, RoadID};

use crate::partition::CustomBoundary;

// Back to basics: flood clumps of local roads, then take convex hull
pub fn partition_v2(map: &Map) -> Vec<CustomBoundary> {
    let mut visited = HashSet::new();

    let mut results = Vec::new();
    for r in map.all_roads() {
        if visited.contains(&r.id) {
            continue;
        }
        if r.is_driveable() && r.get_rank() == RoadRank::Local {
            if let Some(polygon) = floodfill(map, r.id) {
                let custom =
                    polygon_to_custom_boundary(map, polygon, format!("auto {}", results.len()));
                visited.extend(custom.interior_roads.clone());
                results.push(custom);
            }
        }
    }

    results
}

fn floodfill(map: &Map, start: RoadID) -> Option<Polygon> {
    let mut visited = BTreeSet::new();
    let mut queue = vec![start];

    while !queue.is_empty() {
        let current = queue.pop().unwrap();
        if visited.contains(&current) {
            continue;
        }
        visited.insert(current);

        let mut next = Vec::new();
        let mut ok = true;
        for r in map.get_next_roads(current) {
            let r = map.get_r(r);
            if r.is_driveable() && r.get_rank() == RoadRank::Local {
                next.push(r.id);
            }
            if r.get_rank() != RoadRank::Local {
                ok = false;
            }
        }
        if ok {
            queue.extend(next);
        }
    }

    let mut polygons = Vec::new();
    for r in visited {
        polygons.push(map.get_r(r).get_thick_polygon());
    }
    Polygon::convex_hull(polygons).ok()
}

// TODO Dedupe
fn polygon_to_custom_boundary(
    map: &Map,
    boundary_polygon: Polygon,
    name: String,
) -> CustomBoundary {
    let mut interior_roads = BTreeSet::new();
    for r in map.all_roads() {
        if boundary_polygon.intersects_polyline(&r.center_pts) && crate::is_driveable(r, map) {
            interior_roads.insert(r.id);
        }
    }

    // Border intersections are connected to these roads, but not inside the polygon
    let mut borders = BTreeSet::new();
    for r in &interior_roads {
        for i in map.get_r(*r).endpoints() {
            if !boundary_polygon.contains_pt(map.get_i(i).polygon.center()) {
                borders.insert(i);
            }
        }
    }

    CustomBoundary {
        name,
        boundary_polygon,
        borders,
        interior_roads,
    }
}

use abstutil::{retain_btreemap, Timer};
use geom::{Distance, PolyLine, Ring};
use map_model::raw::{OriginalIntersection, OriginalRoad, RawMap};
use map_model::IntersectionType;
use std::collections::BTreeMap;

// TODO This needs to update turn restrictions too
pub fn clip_map(map: &mut RawMap, timer: &mut Timer) {
    timer.start("clipping map to boundary");

    // So we can use retain_btreemap without borrowing issues
    let boundary_polygon = map.boundary_polygon.clone();
    let boundary_ring = Ring::must_new(boundary_polygon.points().clone());

    // This is kind of indirect and slow, but first pass -- just remove roads that start or end
    // outside the boundary polygon.
    retain_btreemap(&mut map.roads, |_, r| {
        let first_in = boundary_polygon.contains_pt(r.center_points[0]);
        let last_in = boundary_polygon.contains_pt(*r.center_points.last().unwrap());
        let light_rail_ok = if r.is_light_rail() {
            // Make sure it's in the boundary somewhere
            r.center_points
                .iter()
                .any(|pt| boundary_polygon.contains_pt(*pt))
        } else {
            false
        };
        first_in || last_in || light_rail_ok
    });

    // When we split an intersection out of bounds into two, one of them gets a new ID. Remember
    // that here.
    let mut extra_borders: BTreeMap<OriginalIntersection, OriginalIntersection> = BTreeMap::new();

    // First pass: Clip roads beginning out of bounds
    let road_ids: Vec<OriginalRoad> = map.roads.keys().cloned().collect();
    for id in road_ids {
        let r = &map.roads[&id];
        if map.boundary_polygon.contains_pt(r.center_points[0]) {
            continue;
        }

        let mut move_i = id.i1;
        let orig_id = id.i1;

        // The road crosses the boundary. If the intersection happens to have another connected
        // road, then we need to copy the intersection before trimming it. This effectively
        // disconnects two roads in the map that would be connected if we left in some
        // partly-out-of-bounds road.
        if map
            .roads
            .keys()
            .filter(|r2| r2.i1 == move_i || r2.i2 == move_i)
            .count()
            > 1
        {
            let copy = map.intersections[&move_i].clone();
            // Don't conflict with OSM IDs
            move_i = OriginalIntersection {
                osm_node_id: map.new_osm_node_id(-1),
            };
            extra_borders.insert(orig_id, move_i);
            map.intersections.insert(move_i, copy);
            println!("Disconnecting {} from some other stuff (starting OOB)", id);
        }

        let i = map.intersections.get_mut(&move_i).unwrap();
        i.intersection_type = IntersectionType::Border;

        // Now trim it.
        let mut mut_r = map.roads.remove(&id).unwrap();
        let center = PolyLine::must_new(mut_r.center_points.clone());
        let border_pt = boundary_ring.all_intersections(&center)[0];
        if let Some(pl) = center.reversed().get_slice_ending_at(border_pt) {
            mut_r.center_points = pl.reversed().into_points();
        } else {
            panic!("{} interacts with border strangely", id);
        }
        i.point = mut_r.center_points[0];
        map.roads.insert(
            OriginalRoad {
                osm_way_id: id.osm_way_id,
                i1: move_i,
                i2: id.i2,
            },
            mut_r,
        );
    }

    // Second pass: clip roads ending out of bounds
    let road_ids: Vec<OriginalRoad> = map.roads.keys().cloned().collect();
    for id in road_ids {
        let r = &map.roads[&id];
        if map
            .boundary_polygon
            .contains_pt(*r.center_points.last().unwrap())
        {
            continue;
        }

        let mut move_i = id.i2;
        let orig_id = id.i2;

        // The road crosses the boundary. If the intersection happens to have another connected
        // road, then we need to copy the intersection before trimming it. This effectively
        // disconnects two roads in the map that would be connected if we left in some
        // partly-out-of-bounds road.
        if map
            .roads
            .keys()
            .filter(|r2| r2.i1 == move_i || r2.i2 == move_i)
            .count()
            > 1
        {
            let copy = map.intersections[&move_i].clone();
            move_i = OriginalIntersection {
                osm_node_id: map.new_osm_node_id(-1),
            };
            extra_borders.insert(orig_id, move_i);
            map.intersections.insert(move_i, copy);
            println!("Disconnecting {} from some other stuff (ending OOB)", id);
        }

        let i = map.intersections.get_mut(&move_i).unwrap();
        i.intersection_type = IntersectionType::Border;

        // Now trim it.
        let mut mut_r = map.roads.remove(&id).unwrap();
        let center = PolyLine::must_new(mut_r.center_points.clone());
        let border_pt = boundary_ring.all_intersections(&center.reversed())[0];
        if let Some(pl) = center.get_slice_ending_at(border_pt) {
            mut_r.center_points = pl.into_points();
        } else {
            panic!("{} interacts with border strangely", id);
        }
        i.point = *mut_r.center_points.last().unwrap();
        map.roads.insert(
            OriginalRoad {
                osm_way_id: id.osm_way_id,
                i1: id.i1,
                i2: move_i,
            },
            mut_r,
        );
    }

    retain_btreemap(&mut map.buildings, |_, b| {
        b.polygon
            .points()
            .iter()
            .all(|pt| boundary_polygon.contains_pt(*pt))
    });

    let mut result_areas = Vec::new();
    for orig_area in map.areas.drain(..) {
        for polygon in map.boundary_polygon.intersection(&orig_area.polygon) {
            let mut area = orig_area.clone();
            area.polygon = polygon;
            result_areas.push(area);
        }
    }
    map.areas = result_areas;

    // TODO Don't touch parking lots. It'll be visually obvious if a clip intersects one of these.
    // The boundary should be manually adjusted.

    if map.roads.is_empty() {
        panic!("There are no roads inside the clipping polygon");
    }

    let all_routes = map.bus_routes.drain(..).collect::<Vec<_>>();
    for mut r in all_routes {
        if r.stops[0].vehicle_pos == r.stops.last().unwrap().vehicle_pos {
            // A loop?
            map.bus_routes.push(r);
            continue;
        }

        let mut borders: Vec<OriginalIntersection> = Vec::new();
        for pt in &r.all_pts {
            if let Some(i) = map.intersections.get(pt) {
                if i.intersection_type == IntersectionType::Border {
                    borders.push(*pt);
                }
            }
            if let Some(i) = extra_borders.get(pt) {
                borders.push(*i);
            }
        }

        // Guess which border is for the beginning and end of the route.
        let start_i = map.closest_intersection(r.stops[0].vehicle_pos.1);
        let end_i = map.closest_intersection(r.stops.last().unwrap().vehicle_pos.1);
        let mut best_start: Option<(OriginalIntersection, Distance)> = None;
        let mut best_end: Option<(OriginalIntersection, Distance)> = None;
        for i in borders {
            // closest_intersection might snap to the wrong end, so try both directions
            if let Some(d1) = map
                .path_dist_to(i, start_i)
                .or_else(|| map.path_dist_to(start_i, i))
            {
                if best_start.map(|(_, d2)| d1 < d2).unwrap_or(true) {
                    best_start = Some((i, d1));
                }
            }
            if let Some(d1) = map.path_dist_to(end_i, i) {
                if best_end.map(|(_, d2)| d1 < d2).unwrap_or(true) {
                    best_end = Some((i, d1));
                }
            }
        }

        // If both matched to the same border, probably the route properly starts or ends inside
        // the map. (If it was both, then we shouldn't have even had any borders at all.)
        match (best_start, best_end) {
            (Some((i1, d1)), Some((i2, d2))) => {
                if i1 == i2 {
                    if d1 < d2 {
                        r.border_start = Some(i1);
                    } else {
                        r.border_end = Some(i2);
                    }
                } else {
                    r.border_start = Some(i1);
                    r.border_end = Some(i2);
                }
            }
            (Some((i1, _)), None) => {
                r.border_start = Some(i1);
            }
            (None, Some((i2, _))) => {
                r.border_end = Some(i2);
            }
            (None, None) => {}
        }
        map.bus_routes.push(r);
    }

    timer.stop("clipping map to boundary");
}

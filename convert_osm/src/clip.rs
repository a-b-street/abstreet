use abstutil::{retain_btreemap, Timer};
use geom::{PolyLine, Ring};
use map_model::raw::{OriginalIntersection, OriginalRoad, RawMap};
use map_model::IntersectionType;

// TODO This needs to update turn restrictions too
pub fn clip_map(map: &mut RawMap, timer: &mut Timer) {
    timer.start("clipping map to boundary");

    // So we can use retain_btreemap without borrowing issues
    let boundary_polygon = map.boundary_polygon.clone();
    let boundary_ring = Ring::new(boundary_polygon.points().clone());

    // This is kind of indirect and slow, but first pass -- just remove roads that start or end
    // outside the boundary polygon.
    retain_btreemap(&mut map.roads, |_, r| {
        let first_in = boundary_polygon.contains_pt(r.center_points[0]);
        let last_in = boundary_polygon.contains_pt(*r.center_points.last().unwrap());
        first_in || last_in || r.is_light_rail()
    });

    // First pass: Clip roads beginning out of bounds
    let road_ids: Vec<OriginalRoad> = map.roads.keys().cloned().collect();
    for id in road_ids {
        let r = &map.roads[&id];
        if map.boundary_polygon.contains_pt(r.center_points[0]) {
            continue;
        }

        let mut move_i = id.i1;

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
            map.intersections.insert(move_i, copy);
            println!("Disconnecting {} from some other stuff", id);
        }

        let i = map.intersections.get_mut(&move_i).unwrap();
        i.intersection_type = IntersectionType::Border;

        // Now trim it.
        let mut mut_r = map.roads.remove(&id).unwrap();
        let center = PolyLine::new(mut_r.center_points.clone());
        let border_pt = boundary_ring.all_intersections(&center)[0];
        mut_r.center_points = center
            .reversed()
            .get_slice_ending_at(border_pt)
            .unwrap()
            .reversed()
            .points()
            .clone();
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
            map.intersections.insert(move_i, copy);
            println!("Disconnecting {} from some other stuff", id);
        }

        let i = map.intersections.get_mut(&move_i).unwrap();
        i.intersection_type = IntersectionType::Border;

        // Now trim it.
        let mut mut_r = map.roads.remove(&id).unwrap();
        let center = PolyLine::new(mut_r.center_points.clone());
        let border_pt = boundary_ring.all_intersections(&center.reversed())[0];
        mut_r.center_points = center
            .get_slice_ending_at(border_pt)
            .unwrap()
            .points()
            .clone();
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

    timer.stop("clipping map to boundary");
}

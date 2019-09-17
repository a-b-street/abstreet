use abstutil::{retain_btreemap, Timer};
use geom::PolyLine;
use map_model::{raw_data, IntersectionType};

pub fn clip_map(map: &mut raw_data::Map, timer: &mut Timer) {
    timer.start("clipping map to boundary");

    // So we can use retain_btreemap without borrowing issues
    let boundary_polygon = map.boundary_polygon.clone();
    let boundary_lines: Vec<PolyLine> = map
        .boundary_polygon
        .points()
        .windows(2)
        .map(|pair| PolyLine::new(pair.to_vec()))
        .collect();

    // This is kind of indirect and slow, but first pass -- just remove roads that start or end
    // outside the boundary polygon.
    retain_btreemap(&mut map.roads, |_, r| {
        let first_in = boundary_polygon.contains_pt(r.center_points[0]);
        let last_in = boundary_polygon.contains_pt(*r.center_points.last().unwrap());
        first_in || last_in
    });

    let road_ids: Vec<raw_data::StableRoadID> = map.roads.keys().cloned().collect();
    for id in road_ids {
        let r = &map.roads[&id];
        let first_in = map.boundary_polygon.contains_pt(r.center_points[0]);
        let last_in = map
            .boundary_polygon
            .contains_pt(*r.center_points.last().unwrap());

        // Some roads start and end in-bounds, but dip out of bounds. Leave those alone for now.
        if first_in && last_in {
            continue;
        }

        let mut move_i = if first_in { r.i2 } else { r.i1 };

        // The road crosses the boundary. If the intersection happens to have another connected
        // road, then we need to copy the intersection before trimming it. This effectively
        // disconnects two roads in the map that would be connected if we left in some
        // partly-out-of-bounds road.
        if map
            .roads
            .values()
            .filter(|r2| r2.i1 == move_i || r2.i2 == move_i)
            .count()
            > 1
        {
            let copy = map.intersections[&move_i].clone();
            // Nothing deletes intersections yet, so this is safe.
            move_i = raw_data::StableIntersectionID(map.intersections.len());
            map.intersections.insert(move_i, copy);
            println!("Disconnecting {} from some other stuff", id);
            // We don't need to mark the existing intersection as a border and make sure to split
            // all other roads up too. That'll happen later in this loop.
        }

        let i = map.intersections.get_mut(&move_i).unwrap();
        i.intersection_type = IntersectionType::Border;

        // Now trim it.
        let mut_r = map.roads.get_mut(&id).unwrap();
        let center = PolyLine::new(mut_r.center_points.clone());
        let border_pt = boundary_lines
            .iter()
            .find_map(|l| center.intersection(l).map(|(pt, _)| pt))
            .unwrap();
        if first_in {
            mut_r.center_points = center
                .get_slice_ending_at(border_pt)
                .unwrap()
                .points()
                .clone();
            i.point = *mut_r.center_points.last().unwrap();
            i.orig_id.point = i.point.to_gps(&map.gps_bounds).unwrap();
            // This has no effect unless we made a copy of the intersection to disconnect it from
            // other roads.
            mut_r.i2 = move_i;
        } else {
            mut_r.center_points = center
                .reversed()
                .get_slice_ending_at(border_pt)
                .unwrap()
                .reversed()
                .points()
                .clone();
            i.point = mut_r.center_points[0];
            i.orig_id.point = i.point.to_gps(&map.gps_bounds).unwrap();
            mut_r.i1 = move_i;
        }
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

    if map.roads.is_empty() {
        panic!("There are no roads inside the clipping polygon");
    }

    timer.stop("clipping map to boundary");
}

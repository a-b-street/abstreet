use abstutil::{retain_btreemap, Timer};
use clipping::CPolygon;
use geom::{GPSBounds, PolyLine, Polygon, Pt2D};
use map_model::{raw_data, IntersectionType};

pub fn clip_map(map: &mut raw_data::Map, timer: &mut Timer) -> GPSBounds {
    timer.start("clipping map to boundary");
    let bounds = map.get_gps_bounds();

    let boundary_poly = Polygon::new(&bounds.must_convert(&map.boundary_polygon));
    let boundary_lines: Vec<PolyLine> = boundary_poly
        .points()
        .windows(2)
        .map(|pair| PolyLine::new(pair.to_vec()))
        .collect();

    // This is kind of indirect and slow, but first pass -- just remove roads that start or end
    // outside the boundary polygon.
    retain_btreemap(&mut map.roads, |_, r| {
        let center_pts = bounds.must_convert(&r.points);
        let first_in = boundary_poly.contains_pt(center_pts[0]);
        let last_in = boundary_poly.contains_pt(*center_pts.last().unwrap());
        first_in || last_in
    });

    let road_ids: Vec<raw_data::StableRoadID> = map.roads.keys().cloned().collect();
    for id in road_ids {
        let r = &map.roads[&id];
        let center_pts = bounds.must_convert(&r.points);
        let first_in = boundary_poly.contains_pt(center_pts[0]);
        let last_in = boundary_poly.contains_pt(*center_pts.last().unwrap());

        // Some roads start and end in-bounds, but dip out of bounds. Leave those alone for now.
        if first_in && last_in {
            continue;
        }

        let mut move_i = if first_in { r.i2 } else { r.i1 };

        // The road crosses the boundary. If the intersection happens to have another connected
        // road, then we need to copy the intersection before trimming it. This effectively
        // disconnects too roads in the map that would be connected if we left in some
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

        // Convert the road points to a PolyLine here. Loop roads were breaking!
        let center = PolyLine::new(center_pts);

        // Now trim it.
        let mut_r = map.roads.get_mut(&id).unwrap();
        let border_pt = boundary_lines
            .iter()
            .find_map(|l| center.intersection(l).map(|(pt, _)| pt))
            .unwrap();
        if first_in {
            mut_r.points =
                bounds.must_convert_back(center.get_slice_ending_at(border_pt).unwrap().points());
            i.point = *mut_r.points.last().unwrap();
            // This has no effect unless we made a copy of the intersection to disconnect it from
            // other roads.
            mut_r.i2 = move_i;
        } else {
            mut_r.points = bounds.must_convert_back(
                center
                    .reversed()
                    .get_slice_ending_at(border_pt)
                    .unwrap()
                    .reversed()
                    .points(),
            );
            i.point = mut_r.points[0];
            mut_r.i1 = move_i;
        }
    }

    map.buildings.retain(|b| {
        bounds
            .must_convert(&b.points)
            .into_iter()
            .all(|pt| boundary_poly.contains_pt(pt))
    });

    let mut result_areas = Vec::new();
    for orig_area in map.areas.drain(..) {
        let mut boundary_pts = CPolygon::from_vec(
            &boundary_poly
                .points()
                .into_iter()
                .map(|pt| [pt.x(), pt.y()])
                .collect(),
        );
        let mut area_pts = CPolygon::from_vec(
            &bounds
                .must_convert(&orig_area.points)
                .into_iter()
                .map(|pt| [pt.x(), pt.y()])
                .collect(),
        );
        let results = area_pts.intersection(&mut boundary_pts);
        for pts in results {
            let mut area = orig_area.clone();
            area.points = bounds
                .must_convert_back(&pts.into_iter().map(|pt| Pt2D::new(pt[0], pt[1])).collect());
            if area.points[0] != *area.points.last().unwrap() {
                area.points.push(area.points[0]);
            }
            result_areas.push(area);
        }
    }
    map.areas = result_areas;

    timer.stop("clipping map to boundary");
    bounds
}

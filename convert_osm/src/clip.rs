use abstutil::Timer;
use raw_map::RawMap;

pub fn clip_map(map: &mut RawMap, timer: &mut Timer) {
    import_streets::clip::clip_map(&mut map.streets, timer);

    let boundary_polygon = map.streets.boundary_polygon.clone();

    map.buildings.retain(|_, b| {
        b.polygon
            .points()
            .iter()
            .all(|pt| boundary_polygon.contains_pt(*pt))
    });

    let mut result_areas = Vec::new();
    for orig_area in map.areas.drain(..) {
        for polygon in map
            .streets
            .boundary_polygon
            .intersection(&orig_area.polygon)
        {
            let mut area = orig_area.clone();
            area.polygon = polygon;
            result_areas.push(area);
        }
    }
    map.areas = result_areas;

    // TODO Don't touch parking lots. It'll be visually obvious if a clip intersects one of these.
    // The boundary should be manually adjusted.
}

use crate::raw::RawMap;

/// Experimentally try to merge tiny "roads" that're actually just part of a complicated
/// intersection.
pub fn merge_short_roads(map: &mut RawMap) {
    // An expensive fixed-point approach. When we merge one road, the IDs of some other roads might
    // change, so it's simplest just to start over.
    // TODO But since merge_short_road tells us what road IDs were deleted and created, it wouldn't
    // be hard to make a single pass.
    loop {
        let mut changes = false;
        for (id, road) in &map.roads {
            // See https://wiki.openstreetmap.org/wiki/Proposed_features/junction%3Dintersection
            if road.osm_tags.is("junction", "intersection") {
                let id = *id;
                map.merge_short_road(id).unwrap();
                changes = true;
                break;
            }
        }
        if !changes {
            break;
        }
    }
}

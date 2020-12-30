use crate::raw::{OriginalRoad, RawMap};

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
            // Hardcoding some Montlake intersections to test
            if road.osm_tags.is("junction", "intersection")
                || *id == OriginalRoad::new(459084309, (4550007325, 4550007326))
                || *id == OriginalRoad::new(332060258, (3391701875, 1635790583))
            {
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

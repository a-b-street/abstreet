use std::collections::BTreeSet;

use crate::osm::NodeID;
use crate::raw::RawMap;

/// Merge tiny "roads" that're actually just part of a complicated intersection. Returns all
/// surviving intersections adjacent to one of these merged roads.
pub fn merge_short_roads(map: &mut RawMap) -> BTreeSet<NodeID> {
    let mut merged = BTreeSet::new();

    // An expensive fixed-point approach. When we merge one road, the IDs of some other roads might
    // change, so it's simplest just to start over.
    // TODO But since merge_short_road tells us what road IDs were deleted and created, it wouldn't
    // be hard to make a single pass.
    loop {
        let mut changes = false;
        for (id, road) in map.roads.clone() {
            // See https://wiki.openstreetmap.org/wiki/Proposed_features/junction%3Dintersection
            if road.osm_tags.is("junction", "intersection") {
                match map.merge_short_road(id) {
                    Ok((i, _, _, _)) => {
                        merged.insert(i);
                        changes = true;
                        break;
                    }
                    Err(err) => {
                        warn!("Ignoring junction=intersection: {}", err);
                    }
                }
            }
        }
        if !changes {
            break;
        }
    }

    merged
}

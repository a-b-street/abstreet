use std::collections::BTreeSet;

use geom::Distance;

use crate::osm::NodeID;
use crate::raw::RawMap;

// Manually adjust this to try locally. Need to work through issues with merging before enabling
// generally.
const SHORT_ROAD_THRESHOLD: Distance = Distance::const_meters(0.0);

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
            if road.osm_tags.is("junction", "intersection")
                || map
                    .trimmed_road_geometry(id)
                    .map(|pl| pl.length() < SHORT_ROAD_THRESHOLD)
                    .unwrap_or(false)
            {
                match map.merge_short_road(id) {
                    Ok((i, _, _, _)) => {
                        merged.insert(i);
                        changes = true;
                        break;
                    }
                    Err(err) => {
                        warn!("Not merging short road / junction=intersection: {}", err);
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

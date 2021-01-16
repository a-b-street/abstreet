use std::collections::{BTreeSet, VecDeque};

use geom::Distance;

use crate::osm::NodeID;
use crate::raw::{OriginalRoad, RawMap};

// Manually adjust this to try locally. Need to work through issues with merging before enabling
// generally.
const SHORT_ROAD_THRESHOLD: Distance = Distance::const_meters(0.0);

/// Merge tiny "roads" that're actually just part of a complicated intersection. Returns all
/// surviving intersections adjacent to one of these merged roads.
pub fn merge_short_roads(map: &mut RawMap) -> BTreeSet<NodeID> {
    let mut merged = BTreeSet::new();

    let mut queue: VecDeque<OriginalRoad> = VecDeque::new();
    for r in map.roads.keys() {
        queue.push_back(*r);
    }

    while !queue.is_empty() {
        let id = queue.pop_front().unwrap();

        // The road might've been deleted
        if !map.roads.contains_key(&id) {
            continue;
        }

        // See https://wiki.openstreetmap.org/wiki/Proposed_features/junction%3Dintersection
        if map.roads[&id].osm_tags.is("junction", "intersection")
            || map
                .trimmed_road_geometry(id)
                .map(|pl| pl.length() < SHORT_ROAD_THRESHOLD)
                .unwrap_or(false)
        {
            match map.merge_short_road(id) {
                Ok((i, _, _, new_roads)) => {
                    merged.insert(i);
                    queue.extend(new_roads);
                }
                Err(err) => {
                    warn!("Not merging short road / junction=intersection: {}", err);
                }
            }
        }
    }

    merged
}

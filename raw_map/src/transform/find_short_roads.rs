use abstutil::Timer;
use geom::Distance;

use crate::{IntersectionType, OriginalRoad, RawMap};

/// Combines a few different sources/methods to decide which roads are short. Marks them for
/// merging.
///
/// 1) Anything tagged in OSM
/// 2) Anything a temporary local merge_osm_ways.json file
/// 3) If `consolidate_all` is true, an experimental distance-based heuristic
pub fn find_short_roads(map: &mut RawMap, consolidate_all: bool) -> Vec<OriginalRoad> {
    let mut roads = Vec::new();
    for (id, road) in &map.roads {
        if road.osm_tags.is("junction", "intersection") {
            roads.push(*id);
            continue;
        }

        if consolidate_all && distance_heuristic(*id, map) {
            roads.push(*id);
        }
    }

    // Use this to quickly test overrides to some ways before upstreaming in OSM.
    // Since these IDs might be based on already merged roads, do these last.
    if let Ok(ways) = abstio::maybe_read_json::<Vec<OriginalRoad>>(
        "merge_osm_ways.json".to_string(),
        &mut Timer::throwaway(),
    ) {
        roads.extend(ways);
    }

    for id in &roads {
        map.roads
            .get_mut(id)
            .unwrap()
            .osm_tags
            .insert("junction", "intersection");
    }

    roads
}

fn distance_heuristic(id: OriginalRoad, map: &RawMap) -> bool {
    let road_length = if let Some(pl) = map.trimmed_road_geometry(id) {
        pl.length()
    } else {
        // The road or something near it collapsed down into a single point or something. This can
        // happen while merging several short roads around a single junction.
        return false;
    };

    // Any road anywhere shorter than this should get merged.
    road_length < Distance::meters(5.0)
}

impl RawMap {
    /// A heuristic to find short roads near traffic signals
    pub fn find_traffic_signal_clusters(&mut self) -> Vec<OriginalRoad> {
        let threshold = Distance::meters(20.0);

        // Simplest start: look for short roads connected to traffic signals.
        //
        // (This will miss sequences of short roads with stop signs in between a cluster of traffic
        // signals)
        //
        // After trying out around Loop 101, what we really want to do is find clumps of 2 or 4
        // traffic signals, find all the segments between them, and merge those.
        let mut results = Vec::new();
        for (id, road) in &self.roads {
            if road.osm_tags.is("junction", "intersection") {
                continue;
            }
            let i1 = self.intersections[&id.i1].intersection_type;
            let i2 = self.intersections[&id.i2].intersection_type;
            if i1 == IntersectionType::Border || i2 == IntersectionType::Border {
                continue;
            }
            if i1 != IntersectionType::TrafficSignal && i2 != IntersectionType::TrafficSignal {
                continue;
            }
            if let Ok((pl, _)) = road.get_geometry(*id, &self.config) {
                if pl.length() <= threshold {
                    results.push(*id);
                }
            }
        }

        for id in &results {
            self.roads
                .get_mut(id)
                .unwrap()
                .osm_tags
                .insert("junction", "intersection");
        }
        results
    }
}

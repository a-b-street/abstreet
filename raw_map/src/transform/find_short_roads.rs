use abstio::MapName;
use abstutil::Timer;
use geom::Distance;

use crate::{osm, IntersectionType, OriginalRoad, RawMap};

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

    // Gradually rolling out
    if vec![
        MapName::seattle("montlake"),
        MapName::seattle("downtown"),
        MapName::seattle("lakeslice"),
        MapName::new("us", "phoenix", "tempe"),
        MapName::new("gb", "bristol", "east"),
        //MapName::new("gb", "leeds", "north"),
        //MapName::new("gb", "london", "camden"),
        MapName::new("gb", "london", "kennington"),
        //MapName::new("gb", "london", "southwark"),
        //MapName::new("gb", "manchester", "levenshulme"),
        MapName::new("pl", "krakow", "center"),
    ]
    .contains(&map.name)
    {
        roads.extend(map.find_dog_legs());
    }

    // Use this to quickly test overrides to some ways before upstreaming in OSM.
    // Since these IDs might be based on already merged roads, do these last.
    if let Ok(ways) = abstio::maybe_read_json::<Vec<OriginalRoad>>(
        "merge_osm_ways.json".to_string(),
        &mut Timer::throwaway(),
    ) {
        roads.extend(ways);
    }

    map.mark_short_roads(roads)
}

fn distance_heuristic(id: OriginalRoad, map: &RawMap) -> bool {
    let road_length = if let Ok(pl) = map.trimmed_road_geometry(id) {
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
    fn mark_short_roads(&mut self, list: Vec<OriginalRoad>) -> Vec<OriginalRoad> {
        for id in &list {
            self.roads
                .get_mut(id)
                .unwrap()
                .osm_tags
                .insert("junction", "intersection");
        }
        list
    }

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
            let i1 = &self.intersections[&id.i1];
            let i2 = &self.intersections[&id.i2];
            if i1.is_border() || i2.is_border() {
                continue;
            }
            if i1.intersection_type != IntersectionType::TrafficSignal
                && i2.intersection_type != IntersectionType::TrafficSignal
            {
                continue;
            }
            if let Ok((pl, _)) = self.untrimmed_road_geometry(*id) {
                if pl.length() <= threshold {
                    results.push(*id);
                }
            }
        }

        self.mark_short_roads(results)
    }

    /// A heuristic to find short roads in places that would otherwise be a normal four-way
    /// intersection
    ///
    /// ```text
    ///       |
    ///       |
    /// ---X~~X----
    ///    |
    ///    |
    /// ```
    ///
    /// The ~~ is the short road we want to detect
    pub fn find_dog_legs(&mut self) -> Vec<OriginalRoad> {
        let threshold = Distance::meters(5.0);

        let mut results = Vec::new();
        'ROAD: for id in self.roads.keys() {
            let road_length = if let Ok(pl) = self.trimmed_road_geometry(*id) {
                pl.length()
            } else {
                continue;
            };
            if road_length > threshold {
                continue;
            }

            for i in [id.i1, id.i2] {
                let connections = self.roads_per_intersection(i);
                if connections.len() != 3 {
                    continue 'ROAD;
                }
                for r in &connections {
                    // Are both intersections 3-ways of driveable roads? (Don't even attempt
                    // cycleways yet...)
                    if !self.roads[r].is_driveable(&self.config) {
                        continue 'ROAD;
                    }
                    // Don't do anything near border intersections
                    if self.intersections[&r.i1].is_border()
                        || self.intersections[&r.i2].is_border()
                    {
                        continue 'ROAD;
                    }
                }

                // Don't touch the point where dual carriageways split/join, like
                // https://www.openstreetmap.org/node/496331163
                if dual_carriageway_split(self, connections) {
                    continue 'ROAD;
                }
            }

            results.push(*id);
        }
        self.mark_short_roads(results)
    }
}

// TODO Dedupe with find_divided_highways logic in parking_mapper
fn dual_carriageway_split(map: &RawMap, roads: Vec<OriginalRoad>) -> bool {
    assert_eq!(roads.len(), 3);
    // Look for one-way roads with the same name
    for (r1, r2) in [
        (roads[0], roads[1]),
        (roads[0], roads[2]),
        (roads[1], roads[2]),
    ] {
        let road1 = &map.roads[&r1];
        let road2 = &map.roads[&r2];
        if road1.is_oneway()
            && road2.is_oneway()
            && road1.osm_tags.get(osm::NAME) == road2.osm_tags.get(osm::NAME)
        {
            // If they're about the same angle, it's probably not a join/split
            let within_degrees = 30.0;
            if !road1.angle().approx_eq(road2.angle(), within_degrees) {
                return true;
            }
        }
    }
    false
}

use std::collections::{BTreeSet, VecDeque};

use geom::{Angle, Distance};

use crate::osm;
use crate::osm::NodeID;
use crate::raw::{OriginalRoad, RawMap, RawRoad};

/// Merge tiny "roads" that're actually just part of a complicated intersection. Returns all
/// surviving intersections adjacent to one of these merged roads.
pub fn merge_short_roads(map: &mut RawMap, consolidate_all: bool) -> BTreeSet<NodeID> {
    #![allow(clippy::logic_bug)] // remove once the TODO below is taken care of
    let mut merged = BTreeSet::new();

    let mut queue: VecDeque<OriginalRoad> = VecDeque::new();
    for r in map.roads.keys() {
        queue.push_back(*r);

        // TODO Enable after improving this heuristic.
        if false && connects_dual_carriageway(map, r) {
            debug!("{} connects dual carriageways", r);
        }
    }

    while !queue.is_empty() {
        let id = queue.pop_front().unwrap();

        // The road might've been deleted
        if !map.roads.contains_key(&id) {
            continue;
        }

        if should_merge(map, &id, consolidate_all) {
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

fn should_merge(map: &RawMap, id: &OriginalRoad, consolidate_all: bool) -> bool {
    // See https://wiki.openstreetmap.org/wiki/Proposed_features/junction%3Dintersection
    if map.roads[id].osm_tags.is("junction", "intersection") {
        return true;
    }

    // TODO Keep everything below disabled until merging works better.
    if !consolidate_all {
        return false;
    }

    let road_length = if let Some(pl) = map.trimmed_road_geometry(*id) {
        pl.length()
    } else {
        // The road or something near it collapsed down into a single point or something. This can
        // happen while merging several short roads around a single junction.
        return false;
    };

    // Any road anywhere shorter than this should get merged.
    if road_length < Distance::meters(5.0) {
        return true;
    }

    // Roads connecting dual carriageways can use a longer threshold for merging.
    if connects_dual_carriageway(map, id) && road_length < Distance::meters(10.0) {
        return true;
    }

    false
}

// Does this road go between two divided one-ways? Ideally they're tagged explicitly
// (https://wiki.openstreetmap.org/wiki/Tag:dual_carriageway%3Dyes), but we can also apply simple
// heuristics to guess this.
fn connects_dual_carriageway(map: &RawMap, id: &OriginalRoad) -> bool {
    let connectors_angle = angle(&map.roads[id]);
    // There are false positives like https://www.openstreetmap.org/way/4636259 when we're looking
    // at a segment along a marked dual carriageway. Filter out by requiring the intersecting dual
    // carriageways to differ by a minimum angle.
    let within_degrees = 10.0;

    let mut i1_dual_carriageway = false;
    let mut oneway_names_i1: BTreeSet<String> = BTreeSet::new();
    for r in map.roads_per_intersection(id.i1) {
        let road = &map.roads[&r];
        if r == *id || connectors_angle.approx_eq(angle(road), within_degrees) {
            continue;
        }
        if road.osm_tags.is("dual_carriageway", "yes") {
            i1_dual_carriageway = true;
        }
        if road.osm_tags.is("oneway", "yes") {
            if let Some(name) = road.osm_tags.get(osm::NAME) {
                oneway_names_i1.insert(name.to_string());
            }
        }
    }

    let mut i2_dual_carriageway = false;
    let mut oneway_names_i2: BTreeSet<String> = BTreeSet::new();
    for r in map.roads_per_intersection(id.i2) {
        let road = &map.roads[&r];
        if r == *id || connectors_angle.approx_eq(angle(road), within_degrees) {
            continue;
        }
        if road.osm_tags.is("dual_carriageway", "yes") {
            i2_dual_carriageway = true;
        }
        if road.osm_tags.is("oneway", "yes") {
            if let Some(name) = road.osm_tags.get(osm::NAME) {
                oneway_names_i2.insert(name.to_string());
            }
        }
    }

    (i1_dual_carriageway && i2_dual_carriageway)
        || oneway_names_i1
            .intersection(&oneway_names_i2)
            .next()
            .is_some()
}

fn angle(r: &RawRoad) -> Angle {
    r.center_points[0].angle_to(*r.center_points.last().unwrap())
}

use std::collections::BTreeSet;

use geom::Angle;

use crate::{osm, OriginalRoad, RawMap, RawRoad};

/// Does this road go between two divided one-ways? Ideally they're tagged explicitly
/// (https://wiki.openstreetmap.org/wiki/Tag:dual_carriageway%3Dyes), but we can also apply simple
/// heuristics to guess this.
#[allow(unused)]
pub fn connects_dual_carriageway(map: &RawMap, id: &OriginalRoad) -> bool {
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

use std::collections::BTreeSet;

use geom::Distance;

use crate::make::initial::lane_specs::get_lane_specs_ltr;
use crate::osm::NodeID;
use crate::raw::{OriginalRoad, RawMap, RawRoad};
use crate::{osm, LaneType};

/// Collapse degenerate intersections between two cycleways.
pub fn collapse(raw: &mut RawMap) {
    let mut merge: Vec<NodeID> = Vec::new();
    for id in raw.intersections.keys() {
        let roads = raw.roads_per_intersection(*id);
        if roads.len() == 2 && roads.iter().all(|r| is_cycleway(&raw.roads[r], raw)) {
            merge.push(*id);
        }
    }

    for i in merge {
        collapse_intersection(raw, i);
    }

    // It's possible we need to do this in a fixed-point until there are no changes, but meh.
    // Results look good so far.
}

// Rather bruteforce way of figuring this out... is_cycleway logic lifted from Road, unfortunately.
// Better than repeating the OSM tag log from get_lane_specs_ltr.
fn is_cycleway(road: &RawRoad, raw: &RawMap) -> bool {
    // Don't attempt to merge roads with these. They're usually not filled out for cyclepaths.
    if !road.turn_restrictions.is_empty() || !road.complicated_turn_restrictions.is_empty() {
        return false;
    }

    let mut bike = false;
    for spec in get_lane_specs_ltr(&road.osm_tags, &raw.config) {
        if spec.lt == LaneType::Biking {
            bike = true;
        } else if spec.lt != LaneType::Shoulder {
            return false;
        }
    }
    bike
}

pub fn collapse_intersection(raw: &mut RawMap, i: NodeID) {
    let roads = raw.roads_per_intersection(i);
    assert_eq!(roads.len(), 2);
    let r1 = roads[0];
    let r2 = roads[1];

    // Skip loops; they break. Easiest way to detect is see how many total vertices we've got.
    let mut endpts = BTreeSet::new();
    endpts.insert(r1.i1);
    endpts.insert(r1.i2);
    endpts.insert(r2.i1);
    endpts.insert(r2.i2);
    if endpts.len() != 3 {
        info!("Not collapsing degenerate {}, because it's a loop", i);
        return;
    }

    info!("Collapsing degenerate {}", i);
    raw.intersections.remove(&i).unwrap();
    // We could be more careful merging percent_incline and osm_tags, but in practice, it doesn't
    // matter for the short segments we're merging.
    let mut new_road = raw.roads.remove(&r1).unwrap();
    let mut road2 = raw.roads.remove(&r2).unwrap();

    // There are 4 cases, easy to understand on paper. Preserve the original direction of r1
    let (new_i1, new_i2) = if r1.i2 == r2.i1 {
        new_road.center_points.extend(road2.center_points);
        (r1.i1, r2.i2)
    } else if r1.i2 == r2.i2 {
        road2.center_points.reverse();
        new_road.center_points.extend(road2.center_points);
        (r1.i1, r2.i1)
    } else if r1.i1 == r2.i1 {
        road2.center_points.reverse();
        road2.center_points.extend(new_road.center_points);
        new_road.center_points = road2.center_points;
        (r2.i2, r1.i2)
    } else if r1.i1 == r2.i2 {
        road2.center_points.extend(new_road.center_points);
        new_road.center_points = road2.center_points;
        (r2.i1, r1.i2)
    } else {
        unreachable!()
    };
    // Sanity check
    assert!(i != new_i1 && i != new_i2);
    // When we concatenate the points, the common point will be duplicated
    new_road.center_points.dedup();

    raw.roads.insert(
        OriginalRoad {
            osm_way_id: r1.osm_way_id,
            i1: new_i1,
            i2: new_i2,
        },
        new_road,
    );
}

/// Some cycleways intersect footways with detailed curb mapping. The current rules for figuring
/// out which walking paths also allow bikes are imperfect, so we wind up with short dead-end
/// "stubs." Trim those.
///
/// Also do the same thing for extremely short dead-end service roads.
pub fn trim_deadends(raw: &mut RawMap) {
    let mut remove_roads = BTreeSet::new();
    let mut remove_intersections = BTreeSet::new();
    for id in raw.intersections.keys() {
        let roads = raw.roads_per_intersection(*id);
        if roads.len() != 1 {
            continue;
        }
        let road = &raw.roads[&roads[0]];
        if is_cycleway(road, raw) || is_short_service_road(road) {
            remove_roads.insert(roads[0]);
            remove_intersections.insert(*id);
        }
    }

    for r in remove_roads {
        raw.roads.remove(&r).unwrap();
    }
    for i in remove_intersections {
        raw.delete_intersection(i);
    }

    // It's possible we need to do this in a fixed-point until there are no changes, but meh.
    // Results look good so far.
}

fn is_short_service_road(road: &RawRoad) -> bool {
    road.osm_tags.is(osm::HIGHWAY, "service") && road.length() < Distance::meters(10.0)
}

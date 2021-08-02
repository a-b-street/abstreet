use std::collections::BTreeSet;

use anyhow::Result;

use geom::Distance;

use crate::make::initial::lane_specs::get_lane_specs_ltr;
use crate::osm::NodeID;
use crate::raw::{OriginalRoad, RawMap, RawRoad};
use crate::{osm, IntersectionType, LaneSpec, LaneType};

/// Collapse degenerate intersections:
/// - between two cycleways
/// - when the lane specs match and only "unimportant" OSM tags differ
pub fn collapse(raw: &mut RawMap) {
    let mut merge: Vec<NodeID> = Vec::new();
    for id in raw.intersections.keys() {
        let roads = raw.roads_per_intersection(*id);
        if roads.len() != 2 {
            continue;
        }
        match should_collapse(&raw.roads[&roads[0]], &raw.roads[&roads[1]], raw) {
            Ok(()) => {
                merge.push(*id);
            }
            Err(err) => {
                warn!("Not collapsing degenerate intersection {}: {}", id, err);
            }
        }
    }

    for i in merge {
        collapse_intersection(raw, i);
    }

    // It's possible we need to do this in a fixed-point until there are no changes, but meh.
    // Results look good so far.
}

fn should_collapse(r1: &RawRoad, r2: &RawRoad, raw: &RawMap) -> Result<()> {
    // Don't attempt to merge roads with these.
    if !r1.turn_restrictions.is_empty() || !r1.complicated_turn_restrictions.is_empty() {
        bail!("one road has turn restrictions");
    }
    if !r2.turn_restrictions.is_empty() || !r2.complicated_turn_restrictions.is_empty() {
        bail!("one road has turn restrictions");
    }

    let lanes1 = get_lane_specs_ltr(&r1.osm_tags, &raw.config);
    let lanes2 = get_lane_specs_ltr(&r2.osm_tags, &raw.config);
    if lanes1 != lanes2 {
        bail!("lane specs don't match");
    }

    if r1.get_zorder() != r2.get_zorder() {
        bail!("zorders don't match");
    }

    if is_cycleway(&lanes1) && is_cycleway(&lanes2) {
        return Ok(());
    }

    // Check what OSM tags differ. Explicitly allow some keys. Note that lanes tagging doesn't
    // actually matter, because we check that LaneSpecs match. Nor do things indicating a zorder
    // indirectly, like bridge/tunnel.
    // TODO I get the feeling I'll end up swapping this to explicitly list tags that SHOULD block
    // merging.
    for (k, v1, v2) in r1.osm_tags.diff(&r2.osm_tags) {
        if [
            osm::INFERRED_PARKING,
            osm::INFERRED_SIDEWALKS,
            osm::OSM_WAY_ID,
            osm::PARKING_BOTH,
            osm::PARKING_LEFT,
            osm::PARKING_RIGHT,
            "bicycle",
            "bridge",
            "covered",
            "cycleway",
            "cycleway:both",
            "destination",
            "lanes",
            "lanes:backward",
            "lanes:forward",
            "lit",
            "maxheight",
            "maxspeed:advisory",
            "maxweight",
            "note",
            "old_name",
            "short_name",
            "shoulder",
            "sidewalk",
            "surface",
            "tunnel",
            "wikidata",
            "wikimedia_commons",
            "wikipedia",
        ]
        .contains(&k.as_ref())
        {
            continue;
        }

        // Don't worry about ENDPT_FWD and ENDPT_BACK not matching if there are no turn lanes
        // tagged.
        // TODO We could get fancier and copy values over. We'd have to sometimes flip the
        // direction.
        if k == osm::ENDPT_FWD
            && !r1.osm_tags.contains_key("turn:lanes")
            && !r1.osm_tags.contains_key("turn:lanes:forward")
            && !r2.osm_tags.contains_key("turn:lanes")
            && !r2.osm_tags.contains_key("turn:lanes:forward")
        {
            continue;
        }
        if k == osm::ENDPT_BACK
            && !r1.osm_tags.contains_key("turn:lanes:backward")
            && !r2.osm_tags.contains_key("turn:lanes:backward")
        {
            continue;
        }

        bail!("{} = \"{}\" vs \"{}\"", k, v1, v2);
    }

    Ok(())
}

// Rather bruteforce way of figuring this out... is_cycleway logic lifted from Road, unfortunately.
// Better than repeating the OSM tag log from get_lane_specs_ltr.
fn is_cycleway(lanes: &[LaneSpec]) -> bool {
    let mut bike = false;
    for spec in lanes {
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
    let mut r1 = roads[0];
    let mut r2 = roads[1];

    // We'll keep r1's ID, so it's a little more convenient for debugging to guarantee r1 is the
    // longer piece.
    if raw.roads[&r1].length() < raw.roads[&r2].length() {
        std::mem::swap(&mut r1, &mut r2);
    }

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

const SHORT_THRESHOLD: Distance = Distance::const_meters(10.0);

/// Some cycleways intersect footways with detailed curb mapping. The current rules for figuring
/// out which walking paths also allow bikes are imperfect, so we wind up with short dead-end
/// "stubs." Trim those.
///
/// Also do the same thing for extremely short dead-end service roads.
pub fn trim_deadends(raw: &mut RawMap) {
    let mut remove_roads = BTreeSet::new();
    let mut remove_intersections = BTreeSet::new();
    for (id, i) in &raw.intersections {
        let roads = raw.roads_per_intersection(*id);
        if roads.len() != 1 || i.intersection_type == IntersectionType::Border {
            continue;
        }
        let road = &raw.roads[&roads[0]];
        if road.length() < SHORT_THRESHOLD
            && (is_cycleway(&get_lane_specs_ltr(&road.osm_tags, &raw.config))
                || road.osm_tags.is(osm::HIGHWAY, "service"))
        {
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

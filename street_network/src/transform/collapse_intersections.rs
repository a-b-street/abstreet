use std::collections::BTreeSet;

use anyhow::Result;

use geom::{Distance, Pt2D};

use crate::osm::NodeID;
use crate::{osm, IntersectionType, OriginalRoad, StreetNetwork};

/// Collapse degenerate intersections:
/// - between two cycleways
/// - when the lane specs match and only "unimportant" OSM tags differ
pub fn collapse(raw: &mut StreetNetwork) {
    let mut merge: Vec<NodeID> = Vec::new();
    for id in raw.intersections.keys() {
        let roads = raw.roads_per_intersection(*id);
        if roads.len() != 2 {
            continue;
        }
        match should_collapse(roads[0], roads[1], raw) {
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

fn should_collapse(r1: OriginalRoad, r2: OriginalRoad, raw: &StreetNetwork) -> Result<()> {
    let road1 = &raw.roads[&r1];
    let road2 = &raw.roads[&r2];

    // Don't attempt to merge roads with these.
    if !road1.turn_restrictions.is_empty() || !road1.complicated_turn_restrictions.is_empty() {
        bail!("one road has turn restrictions");
    }
    if !road2.turn_restrictions.is_empty() || !road2.complicated_turn_restrictions.is_empty() {
        bail!("one road has turn restrictions");
    }

    // Avoid two one-ways that point at each other. https://www.openstreetmap.org/node/440979339 is
    // a bizarre example. These are actually blackholed, some problem with service roads.
    if road1.oneway_for_driving().is_some()
        && road2.oneway_for_driving().is_some()
        && r1.i2 == r2.i2
    {
        bail!("oneway roads point at each other");
    }

    if road1.lane_specs_ltr != road2.lane_specs_ltr {
        bail!("lane specs don't match");
    }

    if road1.get_zorder() != road2.get_zorder() {
        bail!("zorders don't match");
    }

    if road1.is_cycleway() && road2.is_cycleway() {
        return Ok(());
    }

    // Check what OSM tags differ. Explicitly allow some keys. Note that lanes tagging doesn't
    // actually matter, because we check that LaneSpecs match. Nor do things indicating a zorder
    // indirectly, like bridge/tunnel.
    // TODO I get the feeling I'll end up swapping this to explicitly list tags that SHOULD block
    // merging.
    for (k, v1, v2) in road1.osm_tags.diff(&road2.osm_tags) {
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
            && !road1.osm_tags.contains_key("turn:lanes")
            && !road1.osm_tags.contains_key("turn:lanes:forward")
            && !road2.osm_tags.contains_key("turn:lanes")
            && !road2.osm_tags.contains_key("turn:lanes:forward")
        {
            continue;
        }
        if k == osm::ENDPT_BACK
            && !road1.osm_tags.contains_key("turn:lanes:backward")
            && !road2.osm_tags.contains_key("turn:lanes:backward")
        {
            continue;
        }

        bail!("{} = \"{}\" vs \"{}\"", k, v1, v2);
    }

    Ok(())
}

pub fn collapse_intersection(raw: &mut StreetNetwork, i: NodeID) {
    let roads = raw.roads_per_intersection(i);
    assert_eq!(roads.len(), 2);
    let mut r1 = roads[0];
    let mut r2 = roads[1];

    // We'll keep r1's way ID, so it's a little more convenient for debugging to guarantee r1 is
    // the longer piece.
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
        new_road.osm_center_points.extend(road2.osm_center_points);
        (r1.i1, r2.i2)
    } else if r1.i2 == r2.i2 {
        road2.osm_center_points.reverse();
        new_road.osm_center_points.extend(road2.osm_center_points);
        (r1.i1, r2.i1)
    } else if r1.i1 == r2.i1 {
        road2.osm_center_points.reverse();
        road2.osm_center_points.extend(new_road.osm_center_points);
        new_road.osm_center_points = road2.osm_center_points;
        (r2.i2, r1.i2)
    } else if r1.i1 == r2.i2 {
        road2.osm_center_points.extend(new_road.osm_center_points);
        new_road.osm_center_points = road2.osm_center_points;
        (r2.i1, r1.i2)
    } else {
        unreachable!()
    };
    // Sanity check
    assert!(i != new_i1 && i != new_i2);
    // Simplify curves and dedupe points. The epsilon was tuned for only one location that was
    // breaking
    let epsilon = 1.0;
    new_road.osm_center_points = Pt2D::simplify_rdp(new_road.osm_center_points, epsilon);

    let new_r1 = OriginalRoad {
        osm_way_id: r1.osm_way_id,
        i1: new_i1,
        i2: new_i2,
    };
    raw.roads.insert(new_r1, new_road);

    // We may need to fix up turn restrictions. r1 and r2 both become new_r1.
    let rewrite = |x: &OriginalRoad| *x == r1 || *x == r2;
    for road in raw.roads.values_mut() {
        for (_, id) in &mut road.turn_restrictions {
            if rewrite(id) {
                *id = new_r1;
            }
        }

        for (id1, id2) in &mut road.complicated_turn_restrictions {
            if rewrite(id1) {
                *id1 = new_r1;
            }
            if rewrite(id2) {
                *id2 = new_r1;
            }
        }
    }
}

const SHORT_THRESHOLD: Distance = Distance::const_meters(30.0);

/// Some cycleways intersect footways with detailed curb mapping. The current rules for figuring
/// out which walking paths also allow bikes are imperfect, so we wind up with short dead-end
/// "stubs." Trim those.
///
/// Also do the same thing for extremely short dead-end service roads.
pub fn trim_deadends(raw: &mut StreetNetwork) {
    let mut remove_roads = BTreeSet::new();
    let mut remove_intersections = BTreeSet::new();
    for (id, i) in &raw.intersections {
        let roads = raw.roads_per_intersection(*id);
        if roads.len() != 1 || i.intersection_type == IntersectionType::Border {
            continue;
        }
        let road = &raw.roads[&roads[0]];
        if road.length() < SHORT_THRESHOLD
            && (road.is_cycleway() || road.osm_tags.is(osm::HIGHWAY, "service"))
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

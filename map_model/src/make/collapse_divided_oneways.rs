use std::collections::HashMap;

use crate::osm;
use crate::osm::NodeID;
use crate::raw::{OriginalRoad, RawMap};

/// OSM models roads with some sort of physical divider as separate one-way roads. These tend to
/// break in A/B Street in various ways, so look for simple cases and collapse them.
pub fn collapse(raw: &mut RawMap) {
    // Find cases where a pair of intersections is linked by two one-way roads with the same name,
    // and collapse those into a simple bidirectional road. Around London, these usually represent
    // places with a small pedestrian crossing island. We can't model those yet, and it's a bit
    // better to just have a regular two-way road.
    let mut one_ways: HashMap<(NodeID, NodeID), OriginalRoad> = HashMap::new();
    let mut loop_pairs: Vec<(OriginalRoad, OriginalRoad)> = Vec::new();
    'ROAD: for (id, road) in &raw.roads {
        if road.osm_tags.is("oneway", "yes") {
            // Have we found the other direction?
            if let Some(other) = one_ways.get(&(id.i2, id.i1)) {
                // Do they have the same name?
                if raw.roads[other].osm_tags.get(osm::NAME) == road.osm_tags.get(osm::NAME) {
                    loop_pairs.push((*id, *other));
                    one_ways.remove(&(id.i2, id.i1));
                    continue 'ROAD;
                }
            }
            one_ways.insert((id.i1, id.i2), *id);
        }
    }

    for (r1, r2) in loop_pairs {
        collapse_loop(raw, r1, r2);
    }
}

fn collapse_loop(raw: &mut RawMap, r1: OriginalRoad, r2: OriginalRoad) {
    // Remove r2, keep r1
    let deleted = raw.roads.remove(&r2).unwrap();
    let road = raw.roads.get_mut(&r1).unwrap();

    // Simplify the geometry of the loop. We could consider some way to "average" the two
    // polylines, but in many of the cases I'm auditing, the geometry of the two one-ways juts out
    // and then back in and kind of doesn't represent reality anyway. So just make a simple
    // straight line.
    road.center_points = vec![road.center_points[0], *road.center_points.last().unwrap()];

    // Merge tags between the two one-ways. Let's do this the "ad-hoc" way. The more rigorous
    // approach would transform into the LaneSpecs, append the two sides, and then overwrite OSM
    // tags with whatever would produce that.
    road.osm_tags.remove("oneway");
    let lanes_forward = road
        .osm_tags
        .get("lanes")
        .and_then(|num| num.parse::<usize>().ok())
        .unwrap_or(1);
    let lanes_backward = deleted
        .osm_tags
        .get("lanes")
        .and_then(|num| num.parse::<usize>().ok())
        .unwrap_or(1);
    road.osm_tags
        .insert("lanes", (lanes_forward + lanes_backward).to_string());
    road.osm_tags
        .insert("lanes:forward", lanes_forward.to_string());
    road.osm_tags
        .insert("lanes:backward", lanes_backward.to_string());
    if road.osm_tags.get("sidewalk") == deleted.osm_tags.get("sidewalk") {
        // If both had "left" (UK) or "right" (US), then we can combine
        road.osm_tags.insert("sidewalk", "both");
    }

    // TODO Preserve turn_restrictions and complicated_turn_restrictions
}

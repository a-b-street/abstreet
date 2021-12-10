use std::collections::HashMap;

use abstutil::Tags;

use crate::make::initial::lane_specs::{assemble_ltr, get_lane_specs_ltr};
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

    // Rather than trying to merge the OSM tags from the two, parse each one into LaneSpecs.
    let lane_specs = assemble_ltr(
        get_lane_specs_ltr(&road.osm_tags, &raw.config),
        get_lane_specs_ltr(&deleted.osm_tags, &raw.config)
            .into_iter()
            .map(|mut spec| {
                spec.dir = spec.dir.opposite();
                spec
            })
            .collect(),
        raw.config.driving_side,
    );

    // Encode the lane specs as JSON and just squish into the OSM tags
    let old_tags = std::mem::replace(&mut road.osm_tags, Tags::empty());
    road.osm_tags
        .insert("abst:lanes", abstutil::to_json(&lane_specs));
    // Copy over just a few values
    for key in ["name", "maxspeed"] {
        if let Some(value) = old_tags.get(key) {
            road.osm_tags.insert(key, value);
        }
    }
    // Be careful with the highway tag -- it might be cycleway.
    for value in [
        old_tags.get(osm::HIGHWAY),
        deleted.osm_tags.get(osm::HIGHWAY),
    ] {
        if let Some(value) = value {
            if value != "cycleway" {
                road.osm_tags.insert(osm::HIGHWAY, value);
            }
        }
    }
    if !road.osm_tags.contains_key(osm::HIGHWAY) {
        // Neither road originally had it? Unlikely, but...
        road.osm_tags.insert(osm::HIGHWAY, "residential");
    }

    // TODO Preserve turn_restrictions and complicated_turn_restrictions
}

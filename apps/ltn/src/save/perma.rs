//! The Proposal struct references IntersectionIDs and RoadIDs, which won't survive OSM updates.
//! Similar to the MapEdits <-> PermanentMapEdits strategy, transform those IDs before saving.
//!
//! Unlike PermanentMapEdits, we don't define a PermanentProposal struct, because to do so for
//! everything it includes would be a nightmare. In particular, Partitioning includes Blocks, which
//! nest RoadIDs deep inside. Instead, play a "runtime reflection" trick:
//!
//! 1) Serialize the Proposal with RoadIDs to JSON
//! 2) Dynamically walk the JSON
//! 3) When the path of a value matches the hardcoded list of patterns in is_road_id and
//!    is_intersection_id, transform to a permanent ID
//! 4) Save the proposal as JSON with that ID instead
//! 5) Do the inverse to later load
//!
//! In practice, this attempt to keep proposals compatible with future basemap updates might be
//! futile. We're embedding loads of details about the partitioning, but not checking that they
//! remain valid after loading. Even splitting one road in two anywhere in the map would likely
//! break things kind of silently. Absolute worst case, we also record an abst_version field so we
//! could manually load the proposal in the correct version, and do something to manually recover
//! an old proposal.
//!
//! Also, the JSON blobs are massive because of the partitioning, so compress everything.

use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;

use map_model::{IntersectionID, Map, RoadID};
use raw_map::osm::NodeID;
use raw_map::OriginalRoad;

use super::Proposal;

pub fn to_permanent(map: &Map, proposal: &Proposal) -> Result<Value> {
    let mut proposal_value = serde_json::to_value(proposal)?;
    walk("", &mut proposal_value, &|path, value| {
        if is_road_id(path) {
            let replace_with = map.get_r(RoadID(value.as_u64().unwrap() as usize)).orig_id;
            *value = serde_json::to_value(&replace_with)?;
        } else if is_intersection_id(path) {
            let replace_with = map
                .get_i(IntersectionID(value.as_u64().unwrap() as usize))
                .orig_id;
            *value = serde_json::to_value(&replace_with)?;
        }
        Ok(())
    })?;
    Ok(proposal_value)
}

pub fn from_permanent(map: &Map, mut proposal_value: Value) -> Result<Proposal> {
    walk("", &mut proposal_value, &|path, value| {
        if is_road_id(path) {
            let orig_id: OriginalRoad = serde_json::from_value(value.clone())?;
            let replace_with = map.find_r_by_osm_id(orig_id)?;
            *value = serde_json::to_value(&replace_with)?;
        } else if is_intersection_id(path) {
            let orig_id: NodeID = serde_json::from_value(value.clone())?;
            let replace_with = map.find_i_by_osm_id(orig_id)?;
            *value = serde_json::to_value(&replace_with)?;
        }
        Ok(())
    })?;
    let result = serde_json::from_value(proposal_value)?;
    Ok(result)
}

fn is_road_id(path: &str) -> bool {
    lazy_static! {
        static ref PATTERNS: Vec<Regex> = vec![
            Regex::new(r"^/modal_filters/roads/\d+/0$").unwrap(),
            Regex::new(r"^/modal_filters/intersections/\d+/1/r1$").unwrap(),
            Regex::new(r"^/modal_filters/intersections/\d+/1/r2$").unwrap(),
            Regex::new(r"^/modal_filters/intersections/\d+/1/group1/y$").unwrap(),
            Regex::new(r"^/modal_filters/intersections/\d+/1/group2/y$").unwrap(),
            // First place a Block is stored
            Regex::new(r"^/partitioning/single_blocks/\d+/perimeter/interior/\d+$").unwrap(),
            Regex::new(r"^/partitioning/single_blocks/\d+/perimeter/roads/\d+/road$").unwrap(),
            // The other
            Regex::new(r"^/partitioning/neighborhoods/\d+/0/perimeter/interior/\d+$").unwrap(),
            Regex::new(r"^/partitioning/neighborhoods/\d+/0/perimeter/roads/\d+/road$").unwrap(),
        ];
    }

    PATTERNS.iter().any(|re| re.is_match(path))
}

fn is_intersection_id(path: &str) -> bool {
    lazy_static! {
        static ref PATTERNS: Vec<Regex> = vec![
            Regex::new(r"^/modal_filters/intersections/\d+/0$").unwrap(),
            Regex::new(r"^/modal_filters/intersections/\d+/1/i$").unwrap(),
        ];
    }

    PATTERNS.iter().any(|re| re.is_match(path))
}

// Note there's no chance to transform keys in a map. So use serialize_btreemap elsewhere to force
// into a list of pairs
fn walk<F: Fn(&str, &mut Value) -> Result<()>>(
    path: &str,
    value: &mut Value,
    transform: &F,
) -> Result<()> {
    match value {
        Value::Array(list) => {
            for (idx, x) in list.into_iter().enumerate() {
                walk(&format!("{}/{}", path, idx), x, transform)?;
            }
            transform(path, value)?;
        }
        Value::Object(map) => {
            for (key, val) in map {
                walk(&format!("{}/{}", path, key), val, transform)?;
            }
            // After recursing, possibly transform this. We turn a number into an object, so to
            // reverse that...
            transform(path, value)?;
        }
        _ => {
            transform(path, value)?;
            // The value may have been transformed into an array or object, but don't walk it.
        }
    }
    Ok(())
}

//! TODO All of this is in flux. Ultimately we should "just" use MapEdits, and squeeze partitioning
//! into that somehow.
//!
//! In the meantime, use the existing PermanentMapEdits structure. For partitioning, use "runtime
//! reflection" magic to transform RoadIDs to OriginalRoads. Defining parallel structures manually
//! would be too tedious.
//!
//! 1) Serialize the Partitioning with RoadIDs to JSON
//! 2) Dynamically walk the JSON
//! 3) When the path of a value matches the hardcoded list of patterns in is_road_id, transform
//!    to a permanent ID
//! 4) Save the proposal as JSON with that ID instead
//! 5) Do the inverse to later load
//!
//! In practice, this attempt to keep proposals compatible with future basemap updates might be
//! futile. We're embedding loads of details about the partitioning, but not checking that they
//! remain valid after loading. Even splitting one road in two anywhere in the map would likely
//! break things kind of silently.
//!
//! Also, the JSON blobs are massive because of the partitioning, so compress everything.

use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;
use serde_json::Value;

use map_model::{Map, OriginalRoad, PermanentMapEdits, RoadID};

use super::Proposal;
use crate::save::Partitioning;

pub fn to_permanent(map: &Map, proposal: &Proposal) -> Result<Value> {
    let mut proposal_value = serde_json::to_value(proposal.edits.to_permanent(map))?;

    // Now handle partitioning
    let mut partitioning_value = serde_json::to_value(&proposal.partitioning)?;
    walk("", &mut partitioning_value, &|path, value| {
        if is_road_id(path) {
            let replace_with = map.get_r(RoadID(value.as_u64().unwrap() as usize)).orig_id;
            *value = serde_json::to_value(&replace_with)?;
        }
        Ok(())
    })?;

    proposal_value
        .as_object_mut()
        .unwrap()
        .insert("partitioning".to_string(), partitioning_value);
    Ok(proposal_value)
}

pub fn from_permanent(map: &Map, mut proposal_value: Value) -> Result<Proposal> {
    // Handle partitioning first
    let mut partitioning_value = proposal_value
        .as_object_mut()
        .unwrap()
        .remove("partitioning")
        .unwrap();
    walk("", &mut partitioning_value, &|path, value| {
        if is_road_id(path) {
            let orig_id: OriginalRoad = serde_json::from_value(value.clone())?;
            let replace_with = map.find_r_by_osm_id(orig_id)?;
            *value = serde_json::to_value(&replace_with)?;
        }
        Ok(())
    })?;
    let partitioning: Partitioning = serde_json::from_value(partitioning_value)?;

    // TODO This repeats a bit of MapEdits code, because we're starting from a Value
    // TODO And it skips the compat code
    let perma_edits: PermanentMapEdits = serde_json::from_value(proposal_value)?;
    let edits = perma_edits.into_edits_permissive(map);

    Ok(Proposal {
        edits,
        partitioning,
    })
}

fn is_road_id(path: &str) -> bool {
    lazy_static! {
        static ref PATTERNS: Vec<Regex> = vec![
            // First place a Block is stored
            Regex::new(r"^/partitioning/single_blocks/\d+/perimeter/interior/\d+$").unwrap(),
            Regex::new(r"^/partitioning/single_blocks/\d+/perimeter/roads/\d+/road$").unwrap(),
            // The other
            Regex::new(r"^/partitioning/neighbourhoods/\d+/0/perimeter/interior/\d+$").unwrap(),
            Regex::new(r"^/partitioning/neighbourhoods/\d+/0/perimeter/roads/\d+/road$").unwrap(),
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

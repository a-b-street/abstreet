use std::collections::BTreeMap;

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use abstio::MapName;
use geom::Speed;

use crate::raw::OriginalRoad;
use crate::{
    osm, AccessRestrictions, Direction, EditCmd, EditRoad, LaneType, Map, PermanentMapEdits, RoadID,
};

/// When the PermanentMapEdits format changes, add a transformation here to automatically convert
/// edits written with the old format.
///
/// This problem is often solved with something like protocol buffers, but the resulting proto
/// usually winds up with permanent legacy fields, unless the changes are purely additive. For
/// example, protobufs wouldn't have helped with the fix_intersection_ids problem. Explicit
/// transformation is easier!
pub fn upgrade(mut value: Value, map: &Map) -> Result<PermanentMapEdits> {
    // c46a74f10f4f1976a48aa8642ac11717d74b262c added an explicit version field. There are a few
    // changes before that.
    if value.get("version").is_none() {
        // I don't remember the previous schema change before this. If someone files a bug and has
        // an older file, can add support for it then.
        fix_offset(&mut value);
        fix_intersection_ids(&mut value);

        value
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), Value::Number(0.into()));
    }
    if value["version"] == Value::Number(0.into()) {
        fix_road_direction(&mut value);
        value
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), Value::Number(1.into()));
    }
    if value["version"] == Value::Number(1.into()) {
        fix_old_lane_cmds(&mut value, map)?;
        value
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), Value::Number(2.into()));
    }
    if value["version"] == Value::Number(2.into()) {
        fix_merge_zones(&mut value);
        value
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), Value::Number(3.into()));
    }
    if value["version"] == Value::Number(3.into()) {
        fix_map_name(&mut value);
        value
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), Value::Number(4.into()));
    }
    if value["version"] == Value::Number(4.into()) {
        fix_phase_to_stage(&mut value);
        value
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), Value::Number(5.into()));
    }
    if value["version"] == Value::Number(5.into()) {
        fix_adaptive_stages(&mut value);
        value
            .as_object_mut()
            .unwrap()
            .insert("version".to_string(), Value::Number(6.into()));
    }

    abstutil::from_json(&value.to_string().into_bytes())
}

// Recursively walks the entire JSON object. Will call transform on all of the map objects. If the
// callback returns true, won't recurse into that map.
fn walk<F: Fn(&mut serde_json::Map<String, Value>) -> bool>(value: &mut Value, transform: &F) {
    match value {
        Value::Array(list) => {
            for x in list {
                walk(x, transform);
            }
        }
        Value::Object(map) => {
            if !(transform)(map) {
                for x in map.values_mut() {
                    walk(x, transform);
                }
            }
        }
        _ => {}
    }
}

// eee179ce8a6c1e6133dc212b73c3f79b11603e82 added an offset_seconds field
fn fix_offset(value: &mut Value) {
    walk(value, &|map| {
        if map.len() == 1 && map.contains_key("TrafficSignal") {
            let ts = map
                .get_mut("TrafficSignal")
                .unwrap()
                .as_object_mut()
                .unwrap();
            if ts.get("offset_seconds").is_none() {
                ts.insert("offset_seconds".to_string(), Value::Number(0.into()));
            }
            true
        } else {
            false
        }
    })
}

// 11cefb118ab353d2e7fa5dceaab614a9b775e6ec changed { "osm_node_id": 123 } to just 123
fn fix_intersection_ids(value: &mut Value) {
    match value {
        Value::Array(list) => {
            for x in list {
                fix_intersection_ids(x);
            }
        }
        Value::Object(map) => {
            if map.len() == 1 && map.contains_key("osm_node_id") {
                *value = Value::Number(map["osm_node_id"].as_i64().unwrap().into());
            } else {
                for x in map.values_mut() {
                    fix_intersection_ids(x);
                }
            }
        }
        _ => {}
    }
}

// b137735e019adbe0f2a7372a579aa987f8496e19 changed direction from a boolean to an enum.
fn fix_road_direction(value: &mut Value) {
    walk(value, &|map| {
        if map.contains_key("num_fwd") {
            map.insert(
                "dir".to_string(),
                if map["fwd"].as_bool().unwrap() {
                    "Fwd".into()
                } else {
                    "Back".into()
                },
            );
            true
        } else {
            false
        }
    });
}

// b6ab06d51a3b22702b66db296ed4dfd27e8403a0 (and adjacent commits) removed some commands that
// target a single lane in favor of a consolidated ChangeRoad.
fn fix_old_lane_cmds(value: &mut Value, map: &Map) -> Result<()> {
    // TODO Can we assume map is in its original state? I don't think so... it may have edits
    // applied, right?

    let mut modified: BTreeMap<RoadID, EditRoad> = BTreeMap::new();
    let mut commands = Vec::new();
    for mut orig in value.as_object_mut().unwrap()["commands"]
        .as_array_mut()
        .unwrap()
        .drain(..)
    {
        let cmd = orig.as_object_mut().unwrap();
        if let Some(obj) = cmd.remove("ChangeLaneType") {
            let obj: ChangeLaneType = serde_json::from_value(obj).unwrap();
            let (r, idx) = obj.id.lookup(map)?;
            let road = modified.entry(r).or_insert_with(|| map.get_r_edit(r));
            if road.lanes_ltr[idx].0 != obj.orig_lt {
                bail!("{:?} lane type has changed", obj);
            }
            road.lanes_ltr[idx].0 = obj.lt;
        } else if let Some(obj) = cmd.remove("ReverseLane") {
            let obj: ReverseLane = serde_json::from_value(obj).unwrap();
            let (r, idx) = obj.l.lookup(map)?;
            let dst_i = map.find_i_by_osm_id(obj.dst_i)?;
            let road = modified.entry(r).or_insert_with(|| map.get_r_edit(r));
            let edits_dir = if dst_i == map.get_r(r).dst_i {
                Direction::Fwd
            } else if dst_i == map.get_r(r).src_i {
                Direction::Back
            } else {
                bail!("{:?}'s road doesn't point to dst_i at all", obj);
            };
            if road.lanes_ltr[idx].1 == edits_dir {
                bail!("{:?}'s road already points to dst_i", obj);
            }
            road.lanes_ltr[idx].1 = edits_dir;
        } else if let Some(obj) = cmd.remove("ChangeSpeedLimit") {
            let obj: ChangeSpeedLimit = serde_json::from_value(obj).unwrap();
            let r = map.find_r_by_osm_id(obj.id)?;
            let road = modified.entry(r).or_insert_with(|| map.get_r_edit(r));
            if road.speed_limit != obj.old {
                bail!("{:?} speed limit has changed", obj);
            }
            road.speed_limit = obj.new;
        } else if let Some(obj) = cmd.remove("ChangeAccessRestrictions") {
            let obj: ChangeAccessRestrictions = serde_json::from_value(obj).unwrap();
            let r = map.find_r_by_osm_id(obj.id)?;
            let road = modified.entry(r).or_insert_with(|| map.get_r_edit(r));
            if road.access_restrictions != obj.old {
                bail!("{:?} access restrictions have changed", obj);
            }
            road.access_restrictions = obj.new.clone();
        } else {
            commands.push(orig);
        }
    }

    for (r, new) in modified {
        let old = map.get_r_edit(r);
        commands
            .push(serde_json::to_value(EditCmd::ChangeRoad { r, old, new }.to_perma(map)).unwrap());
    }
    value.as_object_mut().unwrap()["commands"] = Value::Array(commands);
    Ok(())
}

// a3af291b2966c89d63b719e41821705077d063d2 added a map-wide merge_zones field
fn fix_merge_zones(value: &mut Value) {
    let obj = value.as_object_mut().unwrap();
    if !obj.contains_key("merge_zones") {
        obj.insert("merge_zones".to_string(), Value::Bool(true.into()));
    }
}

// fef306489ba5e73735e0badad0172f3992d342db split map/city name into a dedicated struct
fn fix_map_name(value: &mut Value) {
    let root = value.as_object_mut().unwrap();
    if let Value::String(ref name) = root["map_name"].clone() {
        // At the time of this change, there likely aren't many people who have edits saved in
        // other maps.
        root.insert(
            "map_name".to_string(),
            serde_json::to_value(MapName::seattle(name)).unwrap(),
        );
    }
}

// 03fe9400c2ab98b8870e09562b1f35b91036f3cf renamed "phase" to "stage"
fn fix_phase_to_stage(value: &mut Value) {
    walk(value, &|map| {
        if let Some(list) = map.remove("phases") {
            map.insert("stages".to_string(), list);
        }
        if let Some(obj) = map.remove("phase_type") {
            map.insert("stage_type".to_string(), obj);
        }
        false
    });
}

// 34e8b0536a4517c68b0e16e5d55cb5e22dae37d8 remove adaptive signal stages.
fn fix_adaptive_stages(value: &mut Value) {
    walk(value, &|map| {
        if let Some(seconds) = map.remove("Adaptive") {
            // The old adaptive policy would repeat the entire stage if there was any demand at
            // all, so this isn't quite equivalent, since it only doubles the original time at
            // most. This adaptive policy never made any sense, so capturing its behavior more
            // clearly here isn't really worth it.
            let minimum = seconds.clone();
            let delay = Value::Number(1.into());
            let additional = seconds;
            map.insert(
                "Variable".to_string(),
                Value::Array(vec![minimum, delay, additional]),
            );
        }
        false
    });
}

// These're old structs used in fix_old_lane_cmds.
#[derive(Debug, Deserialize)]
struct OriginalLane {
    parent: OriginalRoad,
    num_fwd: usize,
    num_back: usize,
    dir: Direction,
    idx: usize,
}
#[derive(Debug, Deserialize)]
struct ChangeLaneType {
    id: OriginalLane,
    lt: LaneType,
    orig_lt: LaneType,
}
#[derive(Debug, Deserialize)]
struct ReverseLane {
    l: OriginalLane,
    // New intended dst_i
    dst_i: osm::NodeID,
}
#[derive(Debug, Deserialize)]
struct ChangeSpeedLimit {
    id: OriginalRoad,
    new: Speed,
    old: Speed,
}
#[derive(Debug, Deserialize)]
struct ChangeAccessRestrictions {
    id: OriginalRoad,
    new: AccessRestrictions,
    old: AccessRestrictions,
}

impl OriginalLane {
    fn lookup(&self, map: &Map) -> Result<(RoadID, usize)> {
        let r = map.get_r(map.find_r_by_osm_id(self.parent)?);
        let current_fwd = r.children_forwards();
        let current_back = r.children_backwards();
        if current_fwd.len() != self.num_fwd || current_back.len() != self.num_back {
            bail!(
                "number of lanes in {} is ({} fwd, {} back) now, but ({}, {}) in the edits",
                r.orig_id,
                current_fwd.len(),
                current_back.len(),
                self.num_fwd,
                self.num_back
            );
        }
        let l = if self.dir == Direction::Fwd {
            current_fwd[self.idx].0
        } else {
            current_back[self.idx].0
        };
        Ok((r.id, r.offset(l)))
    }
}

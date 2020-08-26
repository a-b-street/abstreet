use crate::PermanentMapEdits;
use serde_json::Value;

// When the PermanentMapEdits format changes, add a transformation here to automatically convert
// edits written with the old format.
//
// This problem is often solved with something like protocol buffers, but the resulting proto
// usually winds up with permanent legacy fields, unless the changes are purely additive. For
// example, protobufs wouldn't have helped with the fix_intersection_ids problem. Explicit
// transformation is easier!
pub fn upgrade(mut value: Value) -> Result<PermanentMapEdits, String> {
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
    }

    abstutil::from_json(&value.to_string().into_bytes()).map_err(|x| x.to_string())
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

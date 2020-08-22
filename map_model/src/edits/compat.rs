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

    abstutil::from_json(&value.to_string().into_bytes()).map_err(|x| x.to_string())
}

// eee179ce8a6c1e6133dc212b73c3f79b11603e82 added an offset_seconds field
fn fix_offset(value: &mut Value) {
    match value {
        Value::Array(list) => {
            for x in list {
                fix_offset(x);
            }
        }
        Value::Object(map) => {
            if map.len() == 1 && map.contains_key("TrafficSignal") {
                let ts = map
                    .get_mut("TrafficSignal")
                    .unwrap()
                    .as_object_mut()
                    .unwrap();
                if ts.get("offset_seconds").is_none() {
                    ts.insert("offset_seconds".to_string(), Value::Number(0.into()));
                }
            } else {
                for x in map.values_mut() {
                    fix_offset(x);
                }
            }
        }
        _ => {}
    }
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

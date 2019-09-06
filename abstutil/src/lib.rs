mod clone;
mod collections;
mod error;
mod io;
mod logs;
mod notes;
mod random;
mod time;

pub use crate::clone::Cloneable;
pub use crate::collections::{
    contains_duplicates, retain_btreemap, wraparound_get, Counter, MultiMap,
};
pub use crate::error::Error;
pub use crate::io::{
    basename, deserialize_btreemap, deserialize_multimap, find_next_file, find_prev_file,
    list_all_objects, list_dir, load_all_objects, read_binary, read_json, save_binary_object,
    save_json_object, serialize_btreemap, serialize_multimap, to_json, write_binary, write_json,
    FileWithProgress,
};
pub use crate::logs::Warn;
pub use crate::notes::note;
pub use crate::random::{fork_rng, WeightedUsizeChoice};
pub use crate::time::{
    elapsed_seconds, prettyprint_usize, MeasureMemory, Profiler, Timer, TimerSink,
};

const PROGRESS_FREQUENCY_SECONDS: f64 = 0.2;

// Thanks https://stackoverflow.com/a/49806368
#[macro_export]
macro_rules! skip_fail {
    ($res:expr) => {
        match $res {
            Some(val) => val,
            None => {
                continue;
            }
        }
    };
}

// TODO It might be nice to organize stuff in data/per_map/. Except it makes looping over all maps
// a bit tougher, and it's unclear how to represent singletons like maps/foo.bin.

pub const AB_TESTS: &str = "ab_tests";
pub const AB_TEST_SAVES: &str = "ab_test_saves";
pub const EDITS: &str = "edits";
pub const NEIGHBORHOODS: &str = "neighborhoods";
pub const POLYGONS: &str = "polygons";
pub const SAVE: &str = "save";
pub const SCENARIOS: &str = "scenarios";
pub const SHORTCUTS: &str = "shortcuts";

pub fn path1(map_name: &str, category: &str, dir: &str) -> String {
    format!("../data/{}/{}/{}", category, map_name, dir)
}

pub fn path1_json(map_name: &str, category: &str, instance: &str) -> String {
    format!("../data/{}/{}/{}.json", category, map_name, instance)
}

pub fn path1_bin(map_name: &str, category: &str, instance: &str) -> String {
    format!("../data/{}/{}/{}.bin", category, map_name, instance)
}

pub fn path2_dir(map_name: &str, category: &str, dir: &str) -> String {
    format!("../data/{}/{}/{}/", category, map_name, dir)
}

pub fn path2_bin(map_name: &str, category: &str, dir: &str, instance: &str) -> String {
    format!("../data/{}/{}/{}/{}.bin", category, map_name, dir, instance)
}

pub fn path_map(map_name: &str) -> String {
    format!("../data/maps/{}.bin", map_name)
}

pub fn path_polygon(polygon_name: &str) -> String {
    format!("../data/polygons/{}.poly", polygon_name)
}

pub fn path_raw_map(map_name: &str) -> String {
    format!("../data/raw_maps/{}.bin", map_name)
}

pub fn path_editor_state(map_name: &str) -> String {
    format!("../data/editor_state/{}.json", map_name)
}

pub fn path_pending_screenshots(map_name: &str) -> String {
    format!("../data/screenshots/pending_{}", map_name)
}

pub fn path_synthetic_map(map_name: &str) -> String {
    format!("../data/synthetic_maps/{}.json", map_name)
}

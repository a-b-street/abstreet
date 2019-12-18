mod cli;
mod clone;
mod collections;
mod error;
mod io;
mod logs;
mod random;
mod time;

pub use crate::cli::CmdArgs;
pub use crate::clone::Cloneable;
pub use crate::collections::{
    contains_duplicates, retain_btreemap, retain_btreeset, wraparound_get, Counter, MultiMap,
    VecMap,
};
pub use crate::error::Error;
pub use crate::io::{
    basename, deserialize_btreemap, deserialize_multimap, find_next_file, find_prev_file,
    list_all_objects, load_all_objects, maybe_read_binary, maybe_read_json, read_binary, read_json,
    serialize_btreemap, serialize_multimap, serialized_size_bytes, to_json, write_binary,
    write_json, FileWithProgress,
};
pub use crate::logs::Warn;
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

// System data (Players can't edit, needed at runtime)

pub fn path_map(map_name: &str) -> String {
    format!("../data/system/maps/{}.bin", map_name)
}
pub fn path_all_maps() -> String {
    format!("../data/system/maps")
}

pub fn path_prebaked_results(map_name: &str, scenario_name: &str) -> String {
    format!(
        "../data/system/prebaked_results/{}/{}.bin",
        map_name, scenario_name
    )
}

pub fn path_scenario(map_name: &str, scenario_name: &str) -> String {
    format!(
        "../data/system/scenarios/{}/{}.bin",
        map_name, scenario_name
    )
}
pub fn path_all_scenarios(map_name: &str) -> String {
    format!("../data/system/scenarios/{}", map_name)
}

pub fn path_synthetic_map(map_name: &str) -> String {
    format!("../data/system/synthetic_maps/{}.json", map_name)
}
pub fn path_all_synthetic_maps() -> String {
    format!("../data/system/synthetic_maps")
}

// Player data (Players edit this)

pub fn path_ab_test(map_name: &str, test_name: &str) -> String {
    format!("../data/player/ab_tests/{}/{}.json", map_name, test_name)
}
pub fn path_all_ab_tests(map_name: &str) -> String {
    format!("../data/player/ab_tests/{}", map_name)
}

pub fn path_ab_test_save(map_name: &str, test_name: &str, time: String) -> String {
    format!(
        "../data/player/ab_test_saves/{}/{}/{}.bin",
        map_name, test_name, time
    )
}
pub fn path_all_ab_test_saves(map_name: &str, test_name: &str) -> String {
    format!("../data/player/ab_test_saves/{}/{}", map_name, test_name)
}

pub fn path_camera_state(map_name: &str) -> String {
    format!("../data/player/camera_state/{}.json", map_name)
}

pub fn path_edits(map_name: &str, edits_name: &str) -> String {
    format!("../data/player/edits/{}/{}.json", map_name, edits_name)
}
pub fn path_all_edits(map_name: &str) -> String {
    format!("../data/player/edits/{}", map_name)
}

pub fn path_save(map_name: &str, edits_name: &str, run_name: &str, time: String) -> String {
    format!(
        "../data/player/saves/{}/{}_{}/{}.bin",
        map_name, edits_name, run_name, time
    )
}
pub fn path_all_saves(map_name: &str, edits_name: &str, run_name: &str) -> String {
    format!(
        "../data/player/saves/{}/{}_{}",
        map_name, edits_name, run_name
    )
}

pub fn path_shortcut(name: &str) -> String {
    format!("../data/player/shortcuts/{}.json", name)
}
pub fn path_all_shortcuts() -> String {
    format!("../data/player/shortcuts")
}

// Input data (For developers to build maps, not needed at runtime)

pub fn path_fixes(name: &str) -> String {
    format!("../data/input/fixes/{}.json", name)
}

pub fn path_neighborhood(map_name: &str, neighborhood: &str) -> String {
    format!(
        "../data/input/neighborhoods/{}/{}.json",
        map_name, neighborhood
    )
}
pub fn path_all_neighborhoods(map_name: &str) -> String {
    format!("../data/input/neighborhoods/{}", map_name)
}

pub fn path_pending_screenshots(map_name: &str) -> String {
    format!("../data/input/screenshots/pending_{}", map_name)
}

pub fn path_polygon(polygon_name: &str) -> String {
    format!("../data/input/polygons/{}.poly", polygon_name)
}

pub fn path_popdat() -> String {
    format!("../data/input/popdat.bin")
}

pub fn path_raw_map(map_name: &str) -> String {
    format!("../data/input/raw_maps/{}.bin", map_name)
}
pub fn path_all_raw_maps() -> String {
    format!("../data/input/raw_maps")
}

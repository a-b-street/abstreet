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
    basename, deserialize_btreemap, deserialize_multimap, file_exists, find_next_file,
    find_prev_file, list_all_objects, list_dir, load_all_objects, maybe_read_binary,
    maybe_read_json, read_binary, read_json, serialize_btreemap, serialize_multimap,
    serialized_size_bytes, slurp_file, to_json, write_binary, write_json, FileWithProgress,
};
pub use crate::logs::Warn;
pub use crate::random::{fork_rng, WeightedUsizeChoice};
pub use crate::time::{
    elapsed_seconds, prettyprint_usize, start_profiler, stop_profiler, MeasureMemory, Profiler,
    Timer, TimerSink,
};
use std::collections::BTreeSet;
use std::fmt::Write;

const PROGRESS_FREQUENCY_SECONDS: f64 = 0.2;

pub fn clamp(x: f64, min: f64, max: f64) -> f64 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

pub fn plain_list_names(names: BTreeSet<String>) -> String {
    let mut s = String::new();
    let len = names.len();
    for (idx, n) in names.into_iter().enumerate() {
        if idx != 0 {
            if idx == len - 1 {
                if len == 2 {
                    write!(s, " and ").unwrap();
                } else {
                    write!(s, ", and ").unwrap();
                }
            } else {
                write!(s, ", ").unwrap();
            }
        }
        write!(s, "{}", n).unwrap();
    }
    s
}

lazy_static::lazy_static! {
    static ref ROOT_DIR: String = {
        // If you're packaging for a release and need the data directory to be in some fixed
        // location: ABST_DATA_DIR=/some/path cargo build ...
        if let Some(dir) = option_env!("ABST_DATA_DIR") {
            dir.trim_end_matches('/').to_string()
        } else if file_exists("data/".to_string()) {
            "data".to_string()
        } else if file_exists("../data/".to_string()) {
            "../data".to_string()
        } else {
            panic!("Can't find the data/ directory");
        }
    };
}

pub fn path<I: Into<String>>(p: I) -> String {
    format!("{}/{}", *ROOT_DIR, p.into())
}

// System data (Players can't edit, needed at runtime)

pub fn path_map(map_name: &str) -> String {
    format!("{}/system/maps/{}.bin", *ROOT_DIR, map_name)
}
pub fn path_all_maps() -> String {
    format!("{}/system/maps", *ROOT_DIR)
}

pub fn path_prebaked_results(map_name: &str, scenario_name: &str) -> String {
    format!(
        "{}/system/prebaked_results/{}/{}.bin",
        *ROOT_DIR, map_name, scenario_name
    )
}

pub fn path_scenario(map_name: &str, scenario_name: &str) -> String {
    format!(
        "{}/system/scenarios/{}/{}.bin",
        *ROOT_DIR, map_name, scenario_name
    )
}
pub fn path_all_scenarios(map_name: &str) -> String {
    format!("{}/system/scenarios/{}", *ROOT_DIR, map_name)
}

pub fn path_synthetic_map(map_name: &str) -> String {
    format!("{}/system/synthetic_maps/{}.json", *ROOT_DIR, map_name)
}
pub fn path_all_synthetic_maps() -> String {
    format!("{}/system/synthetic_maps", *ROOT_DIR)
}

// Player data (Players edit this)

pub fn path_camera_state(map_name: &str) -> String {
    format!("{}/player/camera_state/{}.json", *ROOT_DIR, map_name)
}

pub fn path_edits(map_name: &str, edits_name: &str) -> String {
    format!(
        "{}/player/edits/{}/{}.json",
        *ROOT_DIR, map_name, edits_name
    )
}
pub fn path_all_edits(map_name: &str) -> String {
    format!("{}/player/edits/{}", *ROOT_DIR, map_name)
}

pub fn path_save(map_name: &str, edits_name: &str, run_name: &str, time: String) -> String {
    format!(
        "{}/player/saves/{}/{}_{}/{}.bin",
        *ROOT_DIR, map_name, edits_name, run_name, time
    )
}
pub fn path_all_saves(map_name: &str, edits_name: &str, run_name: &str) -> String {
    format!(
        "{}/player/saves/{}/{}_{}",
        *ROOT_DIR, map_name, edits_name, run_name
    )
}

// Input data (For developers to build maps, not needed at runtime)

pub fn path_pending_screenshots(map_name: &str) -> String {
    format!("{}/input/screenshots/pending_{}", *ROOT_DIR, map_name)
}

pub fn path_popdat() -> String {
    format!("{}/input/seattle/popdat.bin", *ROOT_DIR)
}

pub fn path_raw_map(map_name: &str) -> String {
    format!("{}/input/raw_maps/{}.bin", *ROOT_DIR, map_name)
}
pub fn path_all_raw_maps() -> String {
    format!("{}/input/raw_maps", *ROOT_DIR)
}

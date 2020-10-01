mod cli;
mod collections;
mod io;
mod random;
mod time;

pub use crate::cli::CmdArgs;
pub use crate::collections::{
    contains_duplicates, retain_btreemap, retain_btreeset, wraparound_get, Counter, MultiMap, Tags,
    VecMap,
};
pub use crate::io::{
    basename, delete_file, deserialize_btreemap, deserialize_multimap, deserialize_usize,
    file_exists, find_next_file, find_prev_file, from_json, list_all_objects, list_dir,
    load_all_objects, maybe_read_binary, maybe_read_json, read_binary, read_json, read_object,
    serialize_btreemap, serialize_multimap, serialize_usize, serialized_size_bytes, slurp_file,
    to_json, write_binary, write_json, FileWithProgress,
};
pub use crate::random::{fork_rng, WeightedUsizeChoice};
pub use crate::time::{
    elapsed_seconds, prettyprint_usize, start_profiler, stop_profiler, Parallelism, Profiler,
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
        } else if cfg!(target_arch = "wasm32") {
            "../data".to_string()
        } else if file_exists("data/".to_string()) {
            "data".to_string()
        } else if file_exists("../data/".to_string()) {
            "../data".to_string()
        } else if file_exists("../../data/".to_string()) {
            "../../data".to_string()
        } else {
            panic!("Can't find the data/ directory");
        }
    };

    static ref ROOT_PLAYER_DIR: String = {
        // If you're packaging for a release and want the player's local data directory to be
        // $HOME/.abstreet, set ABST_PLAYER_HOME_DIR=1
        if option_env!("ABST_PLAYER_HOME_DIR").is_some() {
            match std::env::var("HOME") {
                Ok(dir) => format!("{}/.abstreet", dir.trim_end_matches('/')),
                Err(err) => panic!("This build of A/B Street stores player data in $HOME/.abstreet, but $HOME isn't set: {}", err),
            }
        } else if cfg!(target_arch = "wasm32") {
            "../data".to_string()
        } else if file_exists("data/".to_string()) {
            "data".to_string()
        } else if file_exists("../data/".to_string()) {
            "../data".to_string()
        } else if file_exists("../../data/".to_string()) {
            "../../data".to_string()
        } else {
            panic!("Can't find the data/ directory");
        }
    };
}

pub fn path<I: Into<String>>(p: I) -> String {
    let p = p.into();
    if p.starts_with("player/") {
        format!("{}/{}", *ROOT_PLAYER_DIR, p)
    } else {
        format!("{}/{}", *ROOT_DIR, p)
    }
}

// System data (Players can't edit, needed at runtime)

pub fn path_map(map_name: &str) -> String {
    path(format!("system/maps/{}.bin", map_name))
}
pub fn path_all_maps() -> String {
    path("system/maps")
}

pub fn path_prebaked_results(map_name: &str, scenario_name: &str) -> String {
    path(format!(
        "system/prebaked_results/{}/{}.bin",
        map_name, scenario_name
    ))
}

pub fn path_scenario(map_name: &str, scenario_name: &str) -> String {
    // TODO Getting complicated. Sometimes we're trying to load, so we should look for .bin, then
    // .json. But when we're writing a custom scenario, we actually want to write a .bin.
    let bin = path(format!(
        "system/scenarios/{}/{}.bin",
        map_name, scenario_name
    ));
    let json = path(format!(
        "system/scenarios/{}/{}.json",
        map_name, scenario_name
    ));
    if file_exists(&bin) {
        return bin;
    }
    if file_exists(&json) {
        return json;
    }
    bin
}
pub fn path_all_scenarios(map_name: &str) -> String {
    path(format!("system/scenarios/{}", map_name))
}

pub fn path_synthetic_map(map_name: &str) -> String {
    path(format!("system/synthetic_maps/{}.json", map_name))
}
pub fn path_all_synthetic_maps() -> String {
    path("system/synthetic_maps")
}

// Player data (Players edit this)

pub fn path_camera_state(map_name: &str) -> String {
    path(format!("player/camera_state/{}.json", map_name))
}

pub fn path_edits(map_name: &str, edits_name: &str) -> String {
    path(format!("player/edits/{}/{}.json", map_name, edits_name))
}
pub fn path_all_edits(map_name: &str) -> String {
    path(format!("player/edits/{}", map_name))
}

pub fn path_save(map_name: &str, edits_name: &str, run_name: &str, time: String) -> String {
    path(format!(
        "player/saves/{}/{}_{}/{}.bin",
        map_name, edits_name, run_name, time
    ))
}
pub fn path_all_saves(map_name: &str, edits_name: &str, run_name: &str) -> String {
    path(format!(
        "player/saves/{}/{}_{}",
        map_name, edits_name, run_name
    ))
}

// Input data (For developers to build maps, not needed at runtime)

pub fn path_popdat() -> String {
    path("input/seattle/popdat.bin")
}

pub fn path_raw_map(map_name: &str) -> String {
    path(format!("input/raw_maps/{}.bin", map_name))
}
pub fn path_all_raw_maps() -> String {
    path("input/raw_maps")
}

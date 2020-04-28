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
    find_prev_file, list_all_objects, load_all_objects, maybe_read_binary, maybe_read_json,
    read_binary, read_json, serialize_btreemap, serialize_multimap, serialized_size_bytes,
    slurp_file, to_json, write_binary, write_json, FileWithProgress,
};
pub use crate::logs::Warn;
pub use crate::random::{fork_rng, WeightedUsizeChoice};
pub use crate::time::{
    elapsed_seconds, prettyprint_usize, MeasureMemory, Profiler, Timer, TimerSink,
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

pub fn path_fixes(city: &str, map: &str) -> String {
    format!("../data/input/{}/fixes/{}.json", city, map)
}

pub fn path_neighborhood(city_name: &str, map_name: &str, neighborhood: &str) -> String {
    format!(
        "../data/input/{}/neighborhoods/{}/{}.json",
        city_name, map_name, neighborhood
    )
}
pub fn path_all_neighborhoods(city_name: &str, map_name: &str) -> String {
    format!("../data/input/{}/neighborhoods/{}", city_name, map_name)
}

pub fn path_pending_screenshots(map_name: &str) -> String {
    format!("../data/input/screenshots/pending_{}", map_name)
}

// TODO Few callers, and importer just manually builds this path anyway
pub fn path_polygon(city: &str, polygon_name: &str) -> String {
    format!("../data/input/{}/polygons/{}.poly", city, polygon_name)
}

pub fn path_popdat() -> String {
    format!("../data/input/seattle/popdat.bin")
}

pub fn path_raw_map(map_name: &str) -> String {
    format!("../data/input/raw_maps/{}.bin", map_name)
}
pub fn path_all_raw_maps() -> String {
    format!("../data/input/raw_maps")
}

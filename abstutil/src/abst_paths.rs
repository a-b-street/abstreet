//! Generate paths for different A/B Street files

use crate::file_exists;

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

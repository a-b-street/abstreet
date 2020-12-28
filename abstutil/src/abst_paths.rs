//! Generate paths for different A/B Street files

use serde::{Deserialize, Serialize};

use crate::{basename, file_exists, list_all_objects};

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

/// A single map is identified using this. Using a struct makes refactoring later easier, to
/// organize cities hierarchially.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MapName {
    /// The name of the city, in filename-friendly form -- for example, "tel_aviv". In the future,
    /// this field may change to express more geographic hierarchy.
    pub city: String,
    /// The name of the map within the city, in filename-friendly form -- for example, "downtown"
    pub map: String,
}

impl MapName {
    /// Create a MapName from a simple city and map name.
    pub fn new(city: &str, map: &str) -> MapName {
        MapName {
            city: city.to_string(),
            map: map.to_string(),
        }
    }

    /// Convenient constructor for the main city of the game.
    pub fn seattle(map: &str) -> MapName {
        MapName::new("seattle", map)
    }

    /// Stringify the map name for debug messages. Don't implement `std::fmt::Display`, to force
    /// callers to explicitly opt into this description, which could change.
    pub fn describe(&self) -> String {
        format!("{} (in {})", self.map, self.city)
    }

    /// Stringify the map name for filenames.
    pub fn as_filename(&self) -> String {
        format!("{}_{}", self.city, self.map)
    }

    /// Transforms a path to a map back to a MapName. Crashes if the input is strange.
    pub fn from_path(path: &str) -> MapName {
        // TODO regex
        let parts = path.split("/").collect::<Vec<_>>();
        let city = parts[parts.len() - 3];
        let map = basename(parts[parts.len() - 1]);
        MapName::new(city, &map)
    }

    /// Returns the filesystem path to this map.
    pub fn path(&self) -> String {
        path(format!("system/{}/maps/{}.bin", self.city, self.map))
    }

    /// Returns all maps from all cities.
    pub fn list_all_maps() -> Vec<MapName> {
        let mut names = Vec::new();
        for city in list_all_objects(path("system")) {
            if city == "assets" || city == "proposals" {
                continue;
            }
            for map in list_all_objects(path(format!("system/{}/maps", city))) {
                names.push(MapName::new(&city, &map));
            }
        }
        names
    }
}

// System data (Players can't edit, needed at runtime)

pub fn path_prebaked_results(name: &MapName, scenario_name: &str) -> String {
    path(format!(
        "system/{}/prebaked_results/{}/{}.bin",
        name.city, name.map, scenario_name
    ))
}

pub fn path_scenario(name: &MapName, scenario_name: &str) -> String {
    // TODO Getting complicated. Sometimes we're trying to load, so we should look for .bin, then
    // .json. But when we're writing a custom scenario, we actually want to write a .bin.
    let bin = path(format!(
        "system/{}/scenarios/{}/{}.bin",
        name.city, name.map, scenario_name
    ));
    let json = path(format!(
        "system/{}/scenarios/{}/{}.json",
        name.city, name.map, scenario_name
    ));
    if file_exists(&bin) {
        return bin;
    }
    if file_exists(&json) {
        return json;
    }
    bin
}
pub fn path_all_scenarios(name: &MapName) -> String {
    path(format!("system/{}/scenarios/{}", name.city, name.map))
}

/// Extract the map and scenario name from a path. Crashes if the input is strange.
pub fn parse_scenario_path(path: &str) -> (MapName, String) {
    // TODO regex
    let parts = path.split("/").collect::<Vec<_>>();
    let city = parts[parts.len() - 4];
    let map = parts[parts.len() - 2];
    let scenario = basename(parts[parts.len() - 1]);
    let map_name = MapName::new(city, map);
    (map_name, scenario)
}

// Player data (Players edit this)

pub fn path_player<I: Into<String>>(p: I) -> String {
    path(format!("player/{}", p.into()))
}

pub fn path_camera_state(name: &MapName) -> String {
    path(format!(
        "player/camera_state/{}/{}.json",
        name.city, name.map
    ))
}

pub fn path_edits(name: &MapName, edits_name: &str) -> String {
    path(format!(
        "player/edits/{}/{}/{}.json",
        name.city, name.map, edits_name
    ))
}
pub fn path_all_edits(name: &MapName) -> String {
    path(format!("player/edits/{}/{}", name.city, name.map))
}

pub fn path_save(name: &MapName, edits_name: &str, run_name: &str, time: String) -> String {
    path(format!(
        "player/saves/{}/{}/{}_{}/{}.bin",
        name.city, name.map, edits_name, run_name, time
    ))
}
pub fn path_all_saves(name: &MapName, edits_name: &str, run_name: &str) -> String {
    path(format!(
        "player/saves/{}/{}/{}_{}",
        name.city, name.map, edits_name, run_name
    ))
}

// Input data (For developers to build maps, not needed at runtime)

pub fn path_popdat() -> String {
    path("input/seattle/popdat.bin")
}

pub fn path_raw_map(name: &MapName) -> String {
    path(format!("input/{}/raw_maps/{}.bin", name.city, name.map))
}

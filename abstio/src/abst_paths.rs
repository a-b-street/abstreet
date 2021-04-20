//! Generate paths for different A/B Street files

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstutil::basename;

use crate::{file_exists, list_all_objects, Manifest};

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

pub fn path<I: AsRef<str>>(p: I) -> String {
    let p = p.as_ref();
    if p.starts_with("player/") {
        format!("{}/{}", *ROOT_PLAYER_DIR, p)
    } else {
        format!("{}/{}", *ROOT_DIR, p)
    }
}

/// A single city is identified using this.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CityName {
    /// A two letter lowercase country code, from https://en.wikipedia.org/wiki/ISO_3166-1_alpha-2.
    /// To represent imaginary/test cities, use the code `zz`.
    pub country: String,
    /// The name of the city, in filename-friendly form -- for example, "tel_aviv".
    pub city: String,
}

impl CityName {
    /// Create a CityName from a country code and city.
    pub fn new(country: &str, city: &str) -> CityName {
        if country.len() != 2 {
            panic!(
                "CityName::new({}, {}) has a country code that isn't two letters",
                country, city
            );
        }
        CityName {
            country: country.to_string(),
            city: city.to_string(),
        }
    }

    /// Convenient constructor for the main city of the game.
    pub fn seattle() -> CityName {
        CityName::new("us", "seattle")
    }

    /// Returns all city names available locally.
    fn list_all_cities_locally() -> Vec<CityName> {
        let mut cities = Vec::new();
        for country in list_all_objects(path("system")) {
            if country == "assets"
                || country == "extra_fonts"
                || country == "proposals"
                || country == "study_areas"
            {
                continue;
            }
            for city in list_all_objects(path(format!("system/{}", country))) {
                cities.push(CityName::new(&country, &city));
            }
        }
        cities
    }

    /// Returns all city names based on the manifest of available files.
    pub fn list_all_cities_from_manifest(manifest: &Manifest) -> Vec<CityName> {
        let mut cities = Vec::new();
        for path in manifest.entries.keys() {
            if let Some(city) = Manifest::path_to_city(path) {
                cities.push(city);
            }
        }
        // The paths in the manifest are ordered, so the same cities will be adjacent.
        cities.dedup();
        cities
    }

    /// Returns all city names based on importer config.
    pub fn list_all_cities_from_importer_config() -> Vec<CityName> {
        let mut cities = Vec::new();
        for country in list_all_objects("importer/config".to_string()) {
            for city in list_all_objects(format!("importer/config/{}", country)) {
                cities.push(CityName::new(&country, &city));
            }
        }
        cities
    }

    /// Parses a CityName from something like "gb/london"; the inverse of `to_path`.
    pub fn parse(x: &str) -> Result<CityName> {
        let parts = x.split("/").collect::<Vec<_>>();
        if parts.len() != 2 || parts[0].len() != 2 {
            bail!("Bad CityName {}", x);
        }
        Ok(CityName::new(parts[0], parts[1]))
    }

    /// Expresses the city as a path, like "gb/london"; the inverse of `parse`.
    pub fn to_path(&self) -> String {
        format!("{}/{}", self.country, self.city)
    }

    /// Stringify the city name for debug messages. Don't implement `std::fmt::Display`, to force
    /// callers to explicitly opt into this description, which could change.
    pub fn describe(&self) -> String {
        format!("{} ({})", self.city, self.country)
    }

    /// Constructs the path to some city-scoped data/input.
    pub fn input_path<I: AsRef<str>>(&self, file: I) -> String {
        path(format!(
            "input/{}/{}/{}",
            self.country,
            self.city,
            file.as_ref()
        ))
    }
}

/// A single map is identified using this.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MapName {
    pub city: CityName,
    /// The name of the map within the city, in filename-friendly form -- for example, "downtown"
    pub map: String,
}

impl MapName {
    /// Create a MapName from a country code, city, and map name.
    pub fn new(country: &str, city: &str, map: &str) -> MapName {
        MapName {
            city: CityName::new(country, city),
            map: map.to_string(),
        }
    }

    /// Create a MapName from a city and map within that city.
    pub fn from_city(city: &CityName, map: &str) -> MapName {
        MapName::new(&city.country, &city.city, map)
    }

    /// Convenient constructor for the main city of the game.
    pub fn seattle(map: &str) -> MapName {
        MapName::new("us", "seattle", map)
    }

    /// Stringify the map name for debug messages. Don't implement `std::fmt::Display`, to force
    /// callers to explicitly opt into this description, which could change.
    pub fn describe(&self) -> String {
        format!(
            "{} (in {} ({}))",
            self.map, self.city.city, self.city.country
        )
    }

    /// Stringify the map name for filenames.
    pub fn as_filename(&self) -> String {
        format!("{}_{}_{}", self.city.country, self.city.city, self.map)
    }

    /// Transforms a path to a map back to a MapName. Returns `None` if the input is strange.
    pub fn from_path(path: &str) -> Option<MapName> {
        let parts = path.split("/").collect::<Vec<_>>();
        // Expect something ending like system/us/seattle/maps/montlake.bin
        if parts.len() < 5 || parts[parts.len() - 5] != "system" || parts[parts.len() - 2] != "maps"
        {
            return None;
        }
        let country = parts[parts.len() - 4];
        let city = parts[parts.len() - 3];
        let map = basename(parts[parts.len() - 1]);
        Some(MapName::new(country, city, &map))
    }

    /// Returns the filesystem path to this map.
    pub fn path(&self) -> String {
        path(format!(
            "system/{}/{}/maps/{}.bin",
            self.city.country, self.city.city, self.map
        ))
    }

    /// Returns all maps from one city that're available locally.
    pub fn list_all_maps_in_city_locally(city: &CityName) -> Vec<MapName> {
        let mut names = Vec::new();
        for map in list_all_objects(path(format!("system/{}/{}/maps", city.country, city.city))) {
            names.push(MapName {
                city: city.clone(),
                map,
            });
        }
        names
    }

    /// Returns all maps from all cities available locally.
    pub fn list_all_maps_locally() -> Vec<MapName> {
        let mut names = Vec::new();
        for city in CityName::list_all_cities_locally() {
            names.extend(MapName::list_all_maps_in_city_locally(&city));
        }
        names
    }

    /// Returns all maps from all cities based on the manifest of available files.
    pub fn list_all_maps_from_manifest(manifest: &Manifest) -> Vec<MapName> {
        let mut names = Vec::new();
        for path in manifest.entries.keys() {
            if let Some(name) = MapName::from_path(path) {
                names.push(name);
            }
        }
        names
    }

    /// Returns all maps from one city based on the manifest of available files.
    pub fn list_all_maps_in_city_from_manifest(
        city: &CityName,
        manifest: &Manifest,
    ) -> Vec<MapName> {
        let mut names = Vec::new();
        for path in manifest.entries.keys() {
            if let Some(name) = MapName::from_path(path) {
                if &name.city == city {
                    names.push(name);
                }
            }
        }
        names
    }

    /// Returns the string to opt into runtime or input files for DataPacks.
    pub fn to_data_pack_name(&self) -> String {
        if Manifest::is_file_part_of_huge_seattle(&self.path()) {
            return "us/huge_seattle".to_string();
        }
        self.city.to_path()
    }
}

// System data (Players can't edit, needed at runtime)

pub fn path_prebaked_results(name: &MapName, scenario_name: &str) -> String {
    path(format!(
        "system/{}/{}/prebaked_results/{}/{}.bin",
        name.city.country, name.city.city, name.map, scenario_name
    ))
}

pub fn path_scenario(name: &MapName, scenario_name: &str) -> String {
    // TODO Getting complicated. Sometimes we're trying to load, so we should look for .bin, then
    // .json. But when we're writing a custom scenario, we actually want to write a .bin.
    let bin = path(format!(
        "system/{}/{}/scenarios/{}/{}.bin",
        name.city.country, name.city.city, name.map, scenario_name
    ));
    let json = path(format!(
        "system/{}/{}/scenarios/{}/{}.json",
        name.city.country, name.city.city, name.map, scenario_name
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
    path(format!(
        "system/{}/{}/scenarios/{}",
        name.city.country, name.city.city, name.map
    ))
}

/// Extract the map and scenario name from a path. Crashes if the input is strange.
pub fn parse_scenario_path(path: &str) -> (MapName, String) {
    // TODO regex
    let parts = path.split("/").collect::<Vec<_>>();
    let country = parts[parts.len() - 5];
    let city = parts[parts.len() - 4];
    let map = parts[parts.len() - 2];
    let scenario = basename(parts[parts.len() - 1]);
    let map_name = MapName::new(country, city, map);
    (map_name, scenario)
}

// Player data (Players edit this)

pub fn path_player<I: AsRef<str>>(p: I) -> String {
    path(format!("player/{}", p.as_ref()))
}

pub fn path_camera_state(name: &MapName) -> String {
    path(format!(
        "player/camera_state/{}/{}/{}.json",
        name.city.country, name.city.city, name.map
    ))
}

pub fn path_edits(name: &MapName, edits_name: &str) -> String {
    path(format!(
        "player/edits/{}/{}/{}/{}.json",
        name.city.country, name.city.city, name.map, edits_name
    ))
}
pub fn path_all_edits(name: &MapName) -> String {
    path(format!(
        "player/edits/{}/{}/{}",
        name.city.country, name.city.city, name.map
    ))
}

pub fn path_save(name: &MapName, edits_name: &str, run_name: &str, time: String) -> String {
    path(format!(
        "player/saves/{}/{}/{}/{}_{}/{}.bin",
        name.city.country, name.city.city, name.map, edits_name, run_name, time
    ))
}
pub fn path_all_saves(name: &MapName, edits_name: &str, run_name: &str) -> String {
    path(format!(
        "player/saves/{}/{}/{}/{}_{}",
        name.city.country, name.city.city, name.map, edits_name, run_name
    ))
}

// Input data (For developers to build maps, not needed at runtime)

pub fn path_popdat() -> String {
    path("input/us/seattle/popdat.bin")
}

pub fn path_raw_map(name: &MapName) -> String {
    path(format!(
        "input/{}/{}/raw_maps/{}.bin",
        name.city.country, name.city.city, name.map
    ))
}

pub fn path_shared_input<I: AsRef<str>>(i: I) -> String {
    path(format!("input/shared/{}", i.as_ref()))
}

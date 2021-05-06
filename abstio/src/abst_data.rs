use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::CityName;

/// A list of all canonical data files for A/B Street that're uploaded somewhere. The file formats
/// are tied to the latest version of the git repo. Players use the updater crate to sync these
/// files with local copies.
#[derive(Serialize, Deserialize)]
pub struct Manifest {
    /// Keyed by path, starting with "data/"
    pub entries: BTreeMap<String, Entry>,
}

/// A single file
#[derive(Serialize, Deserialize)]
pub struct Entry {
    /// md5sum of the file
    pub checksum: String,
    /// Uncompressed size in bytes. Because we have some massive files more than 2^32 bytes
    /// described by this, explicitly use u64 instead of usize, so wasm doesn't break.
    pub uncompressed_size_bytes: u64,
    /// Compressed size in bytes
    pub compressed_size_bytes: u64,
}

impl Manifest {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Manifest {
        crate::maybe_read_json(
            crate::path("MANIFEST.json"),
            &mut abstutil::Timer::throwaway(),
        )
        .unwrap()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Manifest {
        abstutil::from_json(&include_bytes!("../../data/MANIFEST.json").to_vec()).unwrap()
    }

    /// Removes entries from the Manifest to match the DataPacks that should exist locally.
    pub fn filter(mut self, data_packs: DataPacks) -> Manifest {
        let mut remove = Vec::new();
        for path in self.entries.keys() {
            if path.starts_with("data/system/extra_fonts") {
                // Always grab all of these
                continue;
            }
            // If the user has opted into any input data at all, we want to grab some of the shared
            // input files.
            if path.starts_with("data/input/shared") {
                // But some of the files are large, so of course we hardcode some more overrides
                // here. Maybe some of this data should be scoped to a country, not a city.
                if path.ends_with("Road Safety Data - Accidents 2019.csv")
                    || path.ends_with("wu03ew_v2.csv")
                    || path.ends_with("zones_core.geojson")
                {
                    if data_packs.input.iter().any(|x| x.starts_with("gb/")) {
                        continue;
                    }
                } else if path.ends_with("kc_2016_lidar.tif")
                    || path.ends_with("seattle_contours.geojson")
                {
                    if data_packs.input.contains("us/seattle") {
                        continue;
                    }
                } else if !data_packs.input.is_empty() {
                    // All of the SRTM files land here. Hard to associate them with a country or
                    // city code, and it seems like we may want to share these between cities.
                    continue;
                }
            }

            let parts = path.split("/").collect::<Vec<_>>();
            let mut data_pack = format!("{}/{}", parts[2], parts[3]);
            if Manifest::is_file_part_of_huge_seattle(path) {
                data_pack = "us/huge_seattle".to_string();
            }
            if parts[1] == "input" {
                if data_packs.input.contains(&data_pack) {
                    continue;
                }
            } else if parts[1] == "system" {
                if data_packs.runtime.contains(&data_pack) {
                    continue;
                }
            } else {
                panic!("Wait what's {}", path);
            }
            remove.push(path.clone());
        }
        for path in remove {
            self.entries.remove(&path).unwrap();
        }
        self
    }

    /// Because there are so many Seattle maps and they're included in the weekly release, managing
    /// the total file size is important. The "us/seattle" data pack only contains small maps; the
    /// "us/huge_seattle" pack has the rest. This returns true for files belonging to
    /// "us/huge_seattle".
    pub fn is_file_part_of_huge_seattle(path: &str) -> bool {
        let path = path
            .strip_prefix(&crate::path(""))
            .or_else(|| path.strip_prefix("data/"))
            .unwrap_or_else(|| path);
        let name = if let Some(x) = path.strip_prefix("system/us/seattle/maps/") {
            x.strip_suffix(".bin").unwrap()
        } else if let Some(x) = path.strip_prefix("system/us/seattle/scenarios/") {
            x.split("/").next().unwrap()
        } else if let Some(x) = path.strip_prefix("system/us/seattle/prebaked_results/") {
            x.split("/").next().unwrap()
        } else {
            return false;
        };
        name == "huge_seattle"
            || name == "north_seattle"
            || name == "south_seattle"
            || name == "west_seattle"
            || name == "udistrict"
    }

    /// If an entry's path is system data, return the city.
    pub fn path_to_city(path: &str) -> Option<CityName> {
        let parts = path.split("/").collect::<Vec<_>>();
        if parts[1] == "system" {
            if parts[2] == "assets"
                || parts[2] == "extra_fonts"
                || parts[2] == "proposals"
                || parts[2] == "study_areas"
            {
                return None;
            }
            return Some(CityName::new(parts[2], parts[3]));
        }
        None
    }
}

/// Player-chosen groups of files to opt into downloading
#[derive(Serialize, Deserialize)]
pub struct DataPacks {
    /// A list of cities to download for using in A/B Street. Expressed the same as
    /// `CityName::to_path`, like "gb/london".
    pub runtime: BTreeSet<String>,
    /// A list of cities to download for running the map importer.
    pub input: BTreeSet<String>,
}

impl DataPacks {
    /// Load the player's config for what files to download, or create the config.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_or_create() -> DataPacks {
        let path = crate::path_player("data.json");
        match crate::maybe_read_json::<DataPacks>(path.clone(), &mut abstutil::Timer::throwaway()) {
            Ok(cfg) => cfg,
            Err(err) => {
                warn!("player/data.json invalid, assuming defaults: {}", err);
                let mut cfg = DataPacks {
                    runtime: BTreeSet::new(),
                    input: BTreeSet::new(),
                };
                cfg.runtime.insert("us/seattle".to_string());
                crate::write_json(path, &cfg);
                cfg
            }
        }
    }

    /// Saves the player's config for what files to download.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) {
        crate::write_json(crate::path_player("data.json"), self);
    }
}

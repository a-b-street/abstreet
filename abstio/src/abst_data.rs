use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
    /// Uncompressed size in bytes
    pub uncompressed_size_bytes: usize,
    /// Compressed size in bytes
    pub compressed_size_bytes: usize,
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
            if Manifest::is_file_part_of_huge_seattle(path)
                && !data_packs.runtime.contains("us/huge_seattle")
            {
                remove.push(path.clone());
                continue;
            }
            if path.starts_with("data/system/extra_fonts") {
                // Always grab all of these
                continue;
            }
            if path.starts_with("data/input/shared") && !data_packs.input.is_empty() {
                // Grab all of these if the user has opted into any input data at all
                continue;
            }

            let parts = path.split("/").collect::<Vec<_>>();
            let city = format!("{}/{}", parts[2], parts[3]);
            if parts[1] == "input" {
                if data_packs.input.contains(&city) {
                    continue;
                }
            } else if parts[1] == "system" {
                if data_packs.runtime.contains(&city) {
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
        let name = if let Some(x) = path.strip_prefix("data/system/us/seattle/maps/") {
            x.strip_suffix(".bin").unwrap()
        } else if let Some(x) = path.strip_prefix("data/system/us/seattle/scenarios/") {
            x.split("/").next().unwrap()
        } else if let Some(x) = path.strip_prefix("data/system/us/seattle/prebaked_results/") {
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
        let path = crate::path("player/data.json");
        match crate::maybe_read_json::<DataPacks>(path.clone(), &mut abstutil::Timer::throwaway()) {
            Ok(mut cfg) => {
                // The game breaks without this required data pack.
                cfg.runtime.insert("us/seattle".to_string());
                cfg
            }
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
}

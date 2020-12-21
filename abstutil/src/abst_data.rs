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
        crate::maybe_read_json(crate::path("MANIFEST.json"), &mut crate::Timer::throwaway())
            .unwrap()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Manifest {
        crate::from_json(&include_bytes!("../../data/MANIFEST.json").to_vec()).unwrap()
    }

    /// Removes entries from the Manifest to match the DataPacks that should exist locally.
    pub fn filter(mut self, data_packs: DataPacks) -> Manifest {
        let mut remove = Vec::new();
        for path in self.entries.keys() {
            // TODO Some hardcoded weird exceptions
            if !data_packs.runtime.contains("huge_seattle")
                && (path == "data/system/seattle/maps/huge_seattle.bin"
                    || path == "data/system/seattle/scenarios/huge_seattle/weekday.bin")
            {
                remove.push(path.clone());
                continue;
            }

            let parts = path.split("/").collect::<Vec<_>>();
            if parts[1] == "input" {
                if data_packs.input.contains(parts[2]) {
                    continue;
                }
            } else if parts[1] == "system" {
                if data_packs.runtime.contains(parts[2]) {
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
}

/// Player-chosen groups of files to opt into downloading
#[derive(Serialize, Deserialize)]
pub struct DataPacks {
    /// A list of cities to download for using in A/B Street.
    pub runtime: BTreeSet<String>,
    /// A list of cities to download for running the map importer.
    pub input: BTreeSet<String>,
}

impl DataPacks {
    /// Load the player's config for what files to download, or create the config.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_or_create() -> DataPacks {
        let path = crate::path("player/data.json");
        match crate::maybe_read_json::<DataPacks>(path.clone(), &mut crate::Timer::throwaway()) {
            Ok(mut cfg) => {
                // The game breaks without this required data pack.
                cfg.runtime.insert("seattle".to_string());
                cfg
            }
            Err(err) => {
                warn!("player/data.json invalid, assuming defaults: {}", err);
                let mut cfg = DataPacks {
                    runtime: BTreeSet::new(),
                    input: BTreeSet::new(),
                };
                cfg.runtime.insert("seattle".to_string());
                crate::write_json(path, &cfg);
                cfg
            }
        }
    }
}

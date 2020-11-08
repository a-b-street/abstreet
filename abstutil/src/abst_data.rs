use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::Timer;

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
    /// Size in bytes
    pub size_bytes: usize,
}

impl Manifest {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Manifest {
        crate::maybe_read_json(crate::path("MANIFEST.json"), &mut Timer::throwaway()).unwrap()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Manifest {
        crate::from_json(&include_bytes!("../../data/MANIFEST.json").to_vec()).unwrap()
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
    pub fn load_or_create() -> DataPacks {
        if cfg!(target_arch = "wasm32") {
            panic!("DataPacks::load_or_create shouldn't be called on wasm");
        }

        let path = crate::path("player/data.json");
        match crate::maybe_read_json::<DataPacks>(path.clone(), &mut Timer::throwaway()) {
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

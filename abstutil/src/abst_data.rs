use std::collections::BTreeMap;

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
    /// Size in bytes
    pub size_bytes: usize,
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
}

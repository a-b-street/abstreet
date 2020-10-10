//! A list of all data files that're part of A/B Street. The updater crate manages this file, either
//! downloading updates or, for developers, uploading them.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
    /// URL to a .zip file containing the one file
    pub dropbox_url: Option<String>,
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

    pub fn contains(&self, path: String) -> bool {
        let path = format!("data/{}", path.strip_prefix(&crate::path("")).unwrap());
        self.entries.contains_key(&path)
    }

    pub fn all_map_names(&self) -> BTreeSet<String> {
        self.entries
            .keys()
            .filter_map(|x| x.strip_prefix("data/system/maps/"))
            .map(|x| crate::basename(x))
            .collect()
    }

    pub fn all_scenario_names(&self, map_name: &str) -> Vec<String> {
        let prefix = format!("data/system/scenarios/{}/", map_name);
        self.entries
            .keys()
            .filter_map(|x| x.strip_prefix(&prefix))
            .map(|x| crate::basename(x))
            .collect()
    }
}

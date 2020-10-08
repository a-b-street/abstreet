// A list of all data files that're part of A/B Street. The updater crate manages this file, either
// downloading updates or, for developers, uploading them.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::Timer;

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    // Keyed by path, starting with "data/"
    pub entries: BTreeMap<String, Entry>,
}

// A single file
#[derive(Serialize, Deserialize)]
pub struct Entry {
    // md5sum of the file
    pub checksum: String,
    // URL to a .zip file containing the one file
    pub dropbox_url: Option<String>,
}

impl Manifest {
    pub fn write(&self, path: String) {
        println!("- Wrote {}", path);
        crate::write_json(path, self);
    }

    pub fn load(path: String) -> Result<Manifest, std::io::Error> {
        crate::maybe_read_json(path, &mut Timer::throwaway())
    }

    pub fn all_map_names(&self) -> BTreeSet<String> {
        self.entries
            .keys()
            .filter_map(|x| x.strip_prefix("data/system/maps/"))
            .map(|x| crate::basename(x))
            .collect()
    }
}

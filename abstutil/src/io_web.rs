//! Since the local filesystem can't be read from a web browser, instead bundle system data files in
//! the WASM binary using include_dir. For now, no support for saving files.

use std::error::Error;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use crate::io::*;
use crate::{Manifest, Timer};

// Bring in everything from data/system/ matching one of the prefixes -- aka, no scenarios, and
// only the smallest map. Everything else has to be dynamically loaded over HTTP.
static SYSTEM_DATA: include_dir::Dir = include_dir::include_dir!(
    "../data/system",
    "assets/",
    "cities/",
    "fonts/",
    "maps/montlake.bin",
    "proposals/"
);

// For file_exists and list_dir only, also check if the file is in the Manifest. The caller has to
// know when to load this remotely, though.

pub fn file_exists<I: Into<String>>(path: I) -> bool {
    let path = path.into();
    SYSTEM_DATA
        .get_file(path.trim_start_matches("../data/system/"))
        .is_some()
        || Manifest::load()
            .entries
            .contains_key(path.trim_start_matches("../"))
}

pub fn list_dir(dir: String) -> Vec<String> {
    let mut results = Vec::new();
    if let Some(dir) = SYSTEM_DATA.get_dir(dir.trim_start_matches("../data/system/")) {
        for f in dir.files() {
            results.push(format!("../data/system/{}", f.path().display()));
        }
    } else {
        error!("Can't list_dir({})", dir);
    }

    // Merge with remote files
    let dir = dir.trim_start_matches("../");
    for f in Manifest::load().entries.keys() {
        if f.starts_with(dir) {
            results.push(format!("../{}", f));
        }
    }
    results.sort();

    results
}

pub fn slurp_file(path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        Ok(raw.contents().to_vec())
    } else {
        Err(format!("Can't slurp_file {}, it doesn't exist", path).into())
    }
}

pub fn maybe_read_binary<T: DeserializeOwned>(
    path: String,
    _timer: &mut Timer,
) -> Result<T, Box<dyn Error>> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        bincode::deserialize(raw.contents()).map_err(|x| x.into())
    } else {
        Err(format!("Can't maybe_read_binary {}, it doesn't exist", path).into())
    }
}

pub fn write_json<T: Serialize>(_path: String, _obj: &T) {
    // TODO not yet
}

pub fn write_binary<T: Serialize>(_path: String, _obj: &T) {
    // TODO
}

pub fn delete_file<I: Into<String>>(_path: I) {}

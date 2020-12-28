//! Since the local filesystem can't be read from a web browser, instead bundle system data files in
//! the WASM binary using include_dir. For now, no support for saving files.

use std::collections::BTreeSet;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub use crate::io::*;
use crate::{path_player, to_json, Manifest, Timer};

// Bring in everything from data/system/ matching one of the prefixes -- aka, no scenarios, and
// only the smallest map. Everything else has to be dynamically loaded over HTTP.
static SYSTEM_DATA: include_dir::Dir = include_dir::include_dir!(
    "../data/system",
    "assets/",
    "proposals/",
    "seattle/city.bin",
    "seattle/maps/montlake.bin",
    // used by tutorial
    "seattle/prebaked_results/montlake/car vs bike contention.bin",
);

// For file_exists and list_dir only, also check if the file is in the Manifest. The caller has to
// know when to load this remotely, though.

pub fn file_exists<I: Into<String>>(path: I) -> bool {
    // TODO Handle player data in local storage
    let path = path.into();
    SYSTEM_DATA
        .get_file(path.trim_start_matches("../data/system/"))
        .is_some()
        || Manifest::load()
            .entries
            .contains_key(path.trim_start_matches("../"))
}

pub fn list_dir(dir: String) -> Vec<String> {
    // TODO Handle player data in local storage
    let mut results = BTreeSet::new();
    if dir == "../data/system" {
        for f in SYSTEM_DATA.files() {
            results.insert(format!("../data/system/{}", f.path().display()));
        }
    } else if let Some(dir) = SYSTEM_DATA.get_dir(dir.trim_start_matches("../data/system/")) {
        for f in dir.files() {
            results.insert(format!("../data/system/{}", f.path().display()));
        }
    } else {
        warn!("list_dir({}): not in SYSTEM_DATA, maybe it's remote", dir);
    }

    // Merge with remote files. Duplicates handled by BTreeSet.
    let mut dir = dir.trim_start_matches("../").to_string();
    if !dir.ends_with("/") {
        dir = format!("{}/", dir);
    }
    for f in Manifest::load().entries.keys() {
        if let Some(path) = f.strip_prefix(&dir) {
            // Just list the things immediately in this directory; don't recurse arbitrarily
            results.insert(format!("../{}{}", dir, path.split("/").next().unwrap()));
        }
    }

    results.into_iter().collect()
}

pub fn slurp_file(path: &str) -> Result<Vec<u8>, String> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        Ok(raw.contents().to_vec())
    } else if path.starts_with(&path_player("")) {
        let window = web_sys::window().ok_or("no window?".to_string())?;
        let storage = window
            .local_storage()
            .map_err(|err| {
                err.as_string()
                    .unwrap_or("local_storage failed".to_string())
            })?
            .ok_or("no local_storage?".to_string())?;
        let string = storage
            .get_item(path)
            .map_err(|err| err.as_string().unwrap_or("get_item failed".to_string()))?
            .ok_or(format!("{} missing from local storage", path))?;
        Ok(string.into_bytes())
    } else {
        Err(format!("Can't slurp_file {}, it doesn't exist", path))
    }
}

pub fn maybe_read_binary<T: DeserializeOwned>(path: String, _: &mut Timer) -> Result<T, String> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        bincode::deserialize(raw.contents()).map_err(|x| x.to_string())
    } else {
        Err(format!(
            "Can't maybe_read_binary {}, it doesn't exist",
            path
        ))
    }
}

pub fn write_json<T: Serialize>(path: String, obj: &T) {
    // Only save for data/player, for now
    if !path.starts_with(&path_player("")) {
        warn!("Not saving {}", path);
        return;
    }

    let window = web_sys::window().unwrap();
    let storage = window.local_storage().unwrap().unwrap();
    storage.set_item(&path, &to_json(obj)).unwrap();
}

pub fn write_binary<T: Serialize>(path: String, _obj: &T) {
    // TODO
    warn!("Not saving {}", path);
}

pub fn delete_file<I: Into<String>>(path: I) {
    // TODO
    warn!("Not deleting {}", path.into());
}

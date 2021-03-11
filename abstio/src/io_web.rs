//! Since the local filesystem can't be read from a web browser, instead bundle system data files in
//! the WASM binary using include_dir. For now, no support for saving files.

use std::collections::BTreeSet;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;

use abstutil::{to_json, Timer};

pub use crate::io::*;
use crate::{path_player, Manifest};

// Bring in all assets (except for music), proposals, and study areas. Everything else has to be
// dynamically loaded over HTTP.
//
// As the number of study areas grows, we might need to load these asynchronously instead of
// bundling.
static SYSTEM_DATA: include_dir::Dir = include_dir::include_dir!(
    "../data/system",
    "assets/",
    "-assets/music/",
    "proposals/",
    "study_areas/"
);

// For file_exists and list_dir only, also check if the file is in the Manifest. The caller has to
// know when to load this remotely, though.

pub fn file_exists<I: AsRef<str>>(path: I) -> bool {
    // TODO Handle player data in local storage
    let path = path.as_ref();
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

pub fn slurp_file<I: AsRef<str>>(path: I) -> Result<Vec<u8>> {
    let path = path.as_ref();

    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        Ok(raw.contents().to_vec())
    } else if path.starts_with(&path_player("")) {
        let window = web_sys::window().ok_or(anyhow!("no window?"))?;
        let storage = window
            .local_storage()
            .map_err(|err| {
                anyhow!(err
                    .as_string()
                    .unwrap_or("local_storage failed".to_string()))
            })?
            .ok_or(anyhow!("no local_storage?"))?;
        let string = storage
            .get_item(&path)
            .map_err(|err| anyhow!(err.as_string().unwrap_or("get_item failed".to_string())))?
            .ok_or(anyhow!("{} missing from local storage", path))?;
        Ok(string.into_bytes())
    } else {
        bail!("Can't slurp_file {}, it doesn't exist", path)
    }
}

pub fn maybe_read_binary<T: DeserializeOwned>(path: String, _: &mut Timer) -> Result<T> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        bincode::deserialize(raw.contents()).map_err(|err| err.as_ref())
    } else {
        bail!("Can't maybe_read_binary {}, it doesn't exist", path)
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

pub fn delete_file<I: AsRef<str>>(path: I) {
    // TODO
    warn!("Not deleting {}", path.as_ref());
}

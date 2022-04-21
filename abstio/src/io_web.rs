//! Since the local filesystem can't be read from a web browser, instead bundle system data files in
//! the WASM binary using include_dir. For now, no support for saving files.

use std::collections::BTreeSet;

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Serialize;

use abstutil::Timer;

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
    let path = path.as_ref();

    if path.starts_with(&path_player("")) {
        let window = web_sys::window().unwrap();
        let storage = window.local_storage().unwrap().unwrap();
        return storage.get_item(path).unwrap().is_some();
    }

    SYSTEM_DATA
        .get_file(path.trim_start_matches("../data/system/"))
        .is_some()
        || Manifest::load()
            .entries
            .contains_key(path.trim_start_matches("../"))
}

pub fn list_dir(dir: String) -> Vec<String> {
    let mut results = BTreeSet::new();
    if dir == "../data/system" {
        for f in SYSTEM_DATA.files() {
            results.insert(format!("../data/system/{}", f.path().display()));
        }
    } else if let Some(dir) = SYSTEM_DATA.get_dir(dir.trim_start_matches("../data/system/")) {
        for f in dir.files() {
            results.insert(format!("../data/system/{}", f.path().display()));
        }
    } else if dir.starts_with(&path_player("")) {
        for key in list_local_storage_keys() {
            if key.starts_with(&dir) {
                results.insert(key);
            }
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
        let string = read_local_storage(path)?;
        // TODO Hack: if it probably wasn't written with write_json, do the base64 decoding. This
        // may not always be appropriate...
        if path.ends_with(".json") {
            Ok(string.into_bytes())
        } else {
            base64::decode(string).map_err(|err| err.into())
        }
    } else {
        bail!("Can't slurp_file {}, it doesn't exist", path)
    }
}

pub fn maybe_read_binary<T: DeserializeOwned>(path: String, _: &mut Timer) -> Result<T> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        bincode::deserialize(raw.contents()).map_err(|err| err.into())
    } else if path.starts_with(&path_player("")) {
        let string = read_local_storage(&path)?;
        bincode::deserialize(&base64::decode(string)?).map_err(|err| err.into())
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
    storage.set_item(&path, &abstutil::to_json(obj)).unwrap();
}

pub fn write_binary<T: Serialize>(path: String, obj: &T) {
    write_raw(path, &abstutil::to_binary(obj)).unwrap();
}

pub fn write_raw(path: String, bytes: &[u8]) -> Result<()> {
    // Only save for data/player, for now
    if !path.starts_with(&path_player("")) {
        bail!("Not saving {}", path);
    }

    let window = web_sys::window().unwrap();
    let storage = window.local_storage().unwrap().unwrap();
    // Local storage only supports strings, so base64 encoding needed
    let encoded = base64::encode(bytes);
    storage
        .set_item(&path, &encoded)
        .map_err(|err| anyhow!(err.as_string().unwrap_or("set_item failed".to_string())))?;
    Ok(())
}

pub fn delete_file<I: AsRef<str>>(path: I) {
    let path = path.as_ref();
    if !path.starts_with(&path_player("")) {
        warn!("Not deleting {}", path);
        return;
    }
    let window = web_sys::window().unwrap();
    let storage = window.local_storage().unwrap().unwrap();
    storage.remove_item(path).unwrap();
}

fn read_local_storage(path: &str) -> Result<String> {
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
    Ok(string)
}

fn list_local_storage_keys() -> Vec<String> {
    let window = web_sys::window().unwrap();
    let storage = window.local_storage().unwrap().unwrap();
    let mut keys = Vec::new();
    for idx in 0..storage.length().unwrap() {
        keys.push(storage.key(idx).unwrap().unwrap());
    }
    keys
}

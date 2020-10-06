use std::collections::BTreeMap;
use std::io::{Error, ErrorKind};

use serde::de::DeserializeOwned;

use crate::{basename, list_dir, maybe_read_binary, parent_path, slurp_file, Timer};

pub fn maybe_read_json<T: DeserializeOwned>(path: String, timer: &mut Timer) -> Result<T, Error> {
    if !path.ends_with(".json") && !path.ends_with(".geojson") {
        panic!("read_json needs {} to end with .json or .geojson", path);
    }

    timer.start(format!("parse {}", path));
    // TODO timer.read_file isn't working here. And we need to call stop() if there's no file.
    let result: Result<T, Error> = slurp_file(&path).and_then(|raw| {
        serde_json::from_slice(&raw).map_err(|err| Error::new(ErrorKind::Other, err))
    });
    timer.stop(format!("parse {}", path));
    result
}

pub fn read_json<T: DeserializeOwned>(path: String, timer: &mut Timer) -> T {
    match maybe_read_json(path.clone(), timer) {
        Ok(obj) => obj,
        Err(err) => panic!("Couldn't read_json({}): {}", path, err),
    }
}

pub fn read_binary<T: DeserializeOwned>(path: String, timer: &mut Timer) -> T {
    match maybe_read_binary(path.clone(), timer) {
        Ok(obj) => obj,
        Err(err) => panic!("Couldn't read_binary({}): {}", path, err),
    }
}

pub fn read_object<T: DeserializeOwned>(path: String, timer: &mut Timer) -> Result<T, Error> {
    if path.ends_with(".bin") {
        maybe_read_binary(path, timer)
    } else {
        maybe_read_json(path, timer)
    }
}

// Keeps file extensions
pub fn find_prev_file(orig: String) -> Option<String> {
    let mut files = list_dir(parent_path(&orig));
    files.reverse();
    files.into_iter().find(|f| *f < orig)
}

pub fn find_next_file(orig: String) -> Option<String> {
    let files = list_dir(parent_path(&orig));
    files.into_iter().find(|f| *f > orig)
}

// Load all serialized things from a directory, return sorted by name, with file extension removed.
// Detects JSON or binary. Filters out broken files.
pub fn load_all_objects<T: DeserializeOwned>(dir: String) -> Vec<(String, T)> {
    let mut timer = Timer::new(format!("load_all_objects from {}", dir));
    let mut tree: BTreeMap<String, T> = BTreeMap::new();
    for path in list_dir(dir) {
        match read_object(path.clone(), &mut timer) {
            Ok(obj) => {
                tree.insert(basename(&path), obj);
            }
            Err(err) => {
                error!("Couldn't load {}: {}", path, err);
            }
        }
    }
    tree.into_iter().collect()
}

// Just list all things from a directory, return sorted by name, with file extension removed.
pub fn list_all_objects(dir: String) -> Vec<String> {
    list_dir(dir).into_iter().map(|x| basename(&x)).collect()
}

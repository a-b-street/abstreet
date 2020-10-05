pub use crate::io::*;
use crate::Timer;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{Error, ErrorKind};
use std::path::Path;

static SYSTEM_DATA: include_dir::Dir = include_dir::include_dir!("../data/system");

pub fn write_json<T: Serialize>(_path: String, _obj: &T) {
    // TODO not yet
}

pub fn slurp_file(path: &str) -> Result<Vec<u8>, Error> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        Ok(raw.contents().to_vec())
    } else {
        Err(Error::new(
            ErrorKind::Other,
            format!("Can't slurp_file {}, it doesn't exist", path),
        ))
    }
}

pub fn write_binary<T: Serialize>(_path: String, _obj: &T) {
    // TODO
}

pub fn maybe_read_binary<T: DeserializeOwned>(
    path: String,
    _timer: &mut Timer,
) -> Result<T, Error> {
    if let Some(raw) = SYSTEM_DATA.get_file(path.trim_start_matches("../data/system/")) {
        let obj: T = bincode::deserialize(raw.contents())
            .map_err(|err| Error::new(ErrorKind::Other, err))?;
        Ok(obj)
    } else {
        Err(Error::new(
            ErrorKind::Other,
            format!("Can't maybe_read_binary {}, it doesn't exist", path),
        ))
    }
}

pub fn list_all_objects(dir: String) -> Vec<String> {
    let mut results = Vec::new();
    if let Some(dir) = SYSTEM_DATA.get_dir(dir.trim_start_matches("../data/system/")) {
        for f in dir.files() {
            results.push(format!("../data/system/{}", f.path().display()));
        }
    } else {
        panic!("Can't list_all_objects in {}", dir);
    }
    results
}

pub fn load_all_objects<T: DeserializeOwned>(_dir: String) -> Vec<(String, T)> {
    // TODO
    Vec::new()
}

pub fn file_exists<I: Into<String>>(path: I) -> bool {
    SYSTEM_DATA
        .get_file(path.into().trim_start_matches("../data/system/"))
        .is_some()
}

pub fn delete_file<I: Into<String>>(_path: I) {}

pub fn list_dir(_dir: &Path) -> Vec<String> {
    Vec::new()
}

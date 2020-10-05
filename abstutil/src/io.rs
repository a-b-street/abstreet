use crate::{list_dir, maybe_read_binary, slurp_file, Timer};
use serde::de::DeserializeOwned;
use std::io::{Error, ErrorKind};

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
    let mut files = list_dir(std::path::Path::new(&orig).parent().unwrap());
    files.reverse();
    files.into_iter().find(|f| *f < orig)
}

pub fn find_next_file(orig: String) -> Option<String> {
    let files = list_dir(std::path::Path::new(&orig).parent().unwrap());
    files.into_iter().find(|f| *f > orig)
}

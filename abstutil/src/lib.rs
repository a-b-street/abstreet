extern crate serde;
extern crate serde_cbor;
extern crate serde_json;

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Write};

pub fn write_json<T: Serialize>(path: &str, obj: &T) -> Result<(), Error> {
    let mut file = File::create(path)?;
    file.write_all(serde_json::to_string_pretty(obj).unwrap().as_bytes())?;
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let obj: T = serde_json::from_str(&contents).unwrap();
    Ok(obj)
}

pub fn write_binary<T: Serialize>(path: &str, obj: &T) -> Result<(), Error> {
    let mut file = File::create(path)?;
    serde_cbor::to_writer(&mut file, obj).map_err(|err| Error::new(ErrorKind::Other, err))
}

pub fn read_binary<T: DeserializeOwned>(path: &str) -> Result<T, Error> {
    let file = File::open(path)?;
    let obj: T = serde_cbor::from_reader(file).map_err(|err| Error::new(ErrorKind::Other, err))?;
    Ok(obj)
}

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate serde;
extern crate serde_json;

use control::{ModifiedStopSign, ModifiedTrafficSignal};
use map_model::IntersectionID;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Error, Read, Write};

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,

    pub traffic_signals: HashMap<IntersectionID, ModifiedTrafficSignal>,
    pub stop_signs: HashMap<IntersectionID, ModifiedStopSign>,
}

pub fn write(path: &str, state: EditorState) -> Result<(), Error> {
    let mut file = File::create(path)?;
    file.write_all(serde_json::to_string_pretty(&state).unwrap().as_bytes())?;
    Ok(())
}

pub fn load(path: &str) -> Result<EditorState, Error> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let state: EditorState = serde_json::from_str(&contents).unwrap();
    Ok(state)
}

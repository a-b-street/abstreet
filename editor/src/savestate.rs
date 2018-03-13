// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate serde;
extern crate serde_json;

use map_model::{IntersectionID, RoadID, TurnID};
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

#[derive(Serialize, Deserialize, Debug)]
pub struct ModifiedTrafficSignal {
    pub cycles: Vec<CycleState>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CycleState {
    pub turns: Vec<TurnID>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ModifiedStopSign {
    pub priority_roads: Vec<RoadID>,
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

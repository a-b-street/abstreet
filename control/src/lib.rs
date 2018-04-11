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

extern crate dimensioned;
extern crate geom;
extern crate map_model;
#[macro_use]
extern crate serde_derive;

use geom::GeomMap;
use map_model::{IntersectionID, Map, TurnID};
use std::collections::HashMap;
use stop_signs::{ControlStopSign, TurnPriority};
use traffic_signals::ControlTrafficSignal;

pub mod stop_signs;
pub mod traffic_signals;

// TODO awful name
pub struct ControlMap {
    pub traffic_signals: HashMap<IntersectionID, ControlTrafficSignal>,
    pub stop_signs: HashMap<IntersectionID, ControlStopSign>,
}

impl ControlMap {
    pub fn new(map: &Map, geom_map: &GeomMap) -> ControlMap {
        let mut ctrl = ControlMap {
            traffic_signals: HashMap::new(),
            stop_signs: HashMap::new(),
        };

        for i in map.all_intersections() {
            if i.has_traffic_signal {
                ctrl.traffic_signals
                    .insert(i.id, ControlTrafficSignal::new(map, i.id, &geom_map));
            } else {
                ctrl.stop_signs
                    .insert(i.id, ControlStopSign::new(map, i.id));
            }
        }

        ctrl
    }

    pub fn get_traffic_signals_savestate(&self) -> HashMap<IntersectionID, ModifiedTrafficSignal> {
        let mut h = HashMap::new();
        for (i, s) in &self.traffic_signals {
            if let Some(state) = s.get_savestate() {
                h.insert(*i, state);
            }
        }
        h
    }

    pub fn get_stop_signs_savestate(&self) -> HashMap<IntersectionID, ModifiedStopSign> {
        let mut h = HashMap::new();
        for (i, s) in &self.stop_signs {
            if let Some(state) = s.get_savestate() {
                h.insert(*i, state);
            }
        }
        h
    }

    pub fn load_savestate(
        &mut self,
        traffic_signals: &HashMap<IntersectionID, ModifiedTrafficSignal>,
        stop_signs: &HashMap<IntersectionID, ModifiedStopSign>,
    ) {
        for (i, s) in traffic_signals {
            self.traffic_signals.get_mut(i).unwrap().load_savestate(s);
        }
        for (i, s) in stop_signs {
            self.stop_signs.get_mut(i).unwrap().load_savestate(s);
        }
    }
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
    pub turns: HashMap<TurnID, TurnPriority>,
}

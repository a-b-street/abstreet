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

use control::traffic_signals::TrafficSignal;
use control::stop_signs::StopSign;
use map_model::{IntersectionID, Map};
use render::DrawMap;
use savestate;
use std::collections::HashMap;

pub mod stop_signs;
pub mod traffic_signals;

// TODO awful name
pub struct ControlMap {
    pub traffic_signals: HashMap<IntersectionID, TrafficSignal>,
    pub stop_signs: HashMap<IntersectionID, StopSign>,
}

impl ControlMap {
    pub fn new(map: &Map, draw_map: &DrawMap) -> ControlMap {
        let mut ctrl = ControlMap {
            traffic_signals: HashMap::new(),
            stop_signs: HashMap::new(),
        };

        for i in map.all_intersections() {
            if i.has_traffic_signal {
                ctrl.traffic_signals
                    .insert(i.id, TrafficSignal::new(map, i.id, &draw_map.turns));
            } else {
                ctrl.stop_signs.insert(i.id, StopSign::new(map, i.id));
            }
        }

        ctrl
    }

    pub fn get_traffic_signals_savestate(
        &self,
    ) -> HashMap<IntersectionID, savestate::ModifiedTrafficSignal> {
        let mut h = HashMap::new();
        for (i, s) in &self.traffic_signals {
            if let Some(state) = s.get_savestate() {
                h.insert(*i, state);
            }
        }
        h
    }

    pub fn get_stop_signs_savestate(&self) -> HashMap<IntersectionID, savestate::ModifiedStopSign> {
        let mut h = HashMap::new();
        for (i, s) in &self.stop_signs {
            if let Some(state) = s.get_savestate() {
                h.insert(*i, state);
            }
        }
        h
    }

    pub fn load_savestate(&mut self, state: &savestate::EditorState) {
        for (i, s) in &state.traffic_signals {
            self.traffic_signals.get_mut(i).unwrap().load_savestate(s);
        }
        for (i, s) in &state.stop_signs {
            self.stop_signs.get_mut(i).unwrap().load_savestate(s);
        }
    }
}

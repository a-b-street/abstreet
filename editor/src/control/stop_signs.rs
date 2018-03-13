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

use map_model::{IntersectionID, Map, RoadID};
use render::DrawTurn;
use savestate;
use std::collections::HashMap;

// This represents a single intersection controlled by a stop sign-like policy. There's some kind
// of stop-vs-priority go for every incoming road.
#[derive(Debug)]
pub struct StopSign {
    intersection: IntersectionID,
    road_has_to_stop: HashMap<RoadID, bool>,
    changed: bool,
}

impl StopSign {
    pub fn new(map: &Map, intersection: IntersectionID) -> StopSign {
        assert!(!map.get_i(intersection).has_traffic_signal);
        StopSign::all_way_stop(map, intersection)
    }

    fn all_way_stop(map: &Map, intersection: IntersectionID) -> StopSign {
        let mut ss = StopSign {
            intersection,
            road_has_to_stop: HashMap::new(),
            changed: false,
        };

        for r in &map.get_roads_to_intersection(intersection) {
            ss.road_has_to_stop.insert(r.id, true);
        }

        ss
    }

    // TODO these should assert the road leads to this intersection

    pub fn is_priority_road(&self, id: RoadID) -> bool {
        !self.road_has_to_stop[&id]
    }

    pub fn could_be_priority_road(&self, _id: RoadID, _turns: &Vec<DrawTurn>) -> bool {
        // TODO have to ignore left turns. or should we just look at the road geometry extended
        // somehow?
        true
    }

    pub fn add_priority_road(&mut self, id: RoadID) {
        assert!(self.road_has_to_stop[&id]);
        self.road_has_to_stop.insert(id, false);
        self.changed = true;
    }

    pub fn remove_priority_road(&mut self, id: RoadID) {
        assert!(!self.road_has_to_stop[&id]);
        self.road_has_to_stop.insert(id, true);
        self.changed = true;
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn get_savestate(&self) -> Option<savestate::ModifiedStopSign> {
        if !self.changed() {
            return None;
        }

        let mut priorities = Vec::new();
        for (r, stop) in &self.road_has_to_stop {
            if !stop {
                priorities.push(*r);
            }
        }

        Some(savestate::ModifiedStopSign {
            priority_roads: priorities,
        })
    }

    pub fn load_savestate(&mut self, state: &savestate::ModifiedStopSign) {
        self.changed = true;
        for has_to_stop in self.road_has_to_stop.values_mut() {
            *has_to_stop = true;
        }
        for r in &state.priority_roads {
            self.road_has_to_stop.insert(*r, false);
        }
    }

    // TODO need to color road icons
}

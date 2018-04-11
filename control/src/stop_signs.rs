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

use ModifiedStopSign;

use geom::GeomMap;
use map_model::{IntersectionID, Map, TurnID};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy, PartialOrd)]
pub enum TurnPriority {
    Stop,
    Yield,
    Priority,
}

// This represents a single intersection controlled by a stop sign-like policy. The turns are
// partitioned into three groups:
//
// 1) Priority turns - these must be non-conflicting, and cars don't have to stop before doing this
//    turn.
// 2) Yields - cars can do this immediately if there are no previously accepted conflicting turns.
//    should maybe check that these turns originate from roads with priority turns.
// 3) Stops - cars must stop before doing this turn, and they are accepted with the lowest priority
#[derive(Debug)]
pub struct ControlStopSign {
    intersection: IntersectionID,
    turns: HashMap<TurnID, TurnPriority>,
    changed: bool,
}

impl ControlStopSign {
    pub fn new(map: &Map, intersection: IntersectionID) -> ControlStopSign {
        assert!(!map.get_i(intersection).has_traffic_signal);
        ControlStopSign::all_way_stop(map, intersection)
    }

    fn all_way_stop(map: &Map, intersection: IntersectionID) -> ControlStopSign {
        let mut ss = ControlStopSign {
            intersection,
            turns: HashMap::new(),
            changed: false,
        };
        for t in &map.get_i(intersection).turns {
            ss.turns.insert(*t, TurnPriority::Stop);
        }
        ss
    }

    pub fn get_priority(&self, turn: TurnID) -> TurnPriority {
        self.turns[&turn]
    }

    pub fn set_priority(&mut self, turn: TurnID, priority: TurnPriority, geom_map: &GeomMap) {
        if priority == TurnPriority::Priority {
            assert!(self.could_be_priority_turn(turn, geom_map));
        }
        self.turns.insert(turn, priority);
        self.changed = true;
    }

    pub fn could_be_priority_turn(&self, id: TurnID, geom_map: &GeomMap) -> bool {
        for (t, pri) in &self.turns {
            if *pri == TurnPriority::Priority
                && geom_map.get_t(id).conflicts_with(geom_map.get_t(*t))
            {
                return false;
            }
        }
        true
    }

    pub fn changed(&self) -> bool {
        // TODO detect edits that've been undone, equivalent to original
        self.changed
    }

    pub fn get_savestate(&self) -> Option<ModifiedStopSign> {
        if !self.changed() {
            return None;
        }

        Some(ModifiedStopSign {
            turns: self.turns.clone(),
        })
    }

    pub fn load_savestate(&mut self, state: &ModifiedStopSign) {
        self.changed = true;
        self.turns = state.turns.clone();
    }

    // TODO need to color turn icons
}

#[cfg(test)]
mod tests {
    #[test]
    fn ordering() {
        use stop_signs::TurnPriority;
        assert!(TurnPriority::Priority > TurnPriority::Yield);
    }
}

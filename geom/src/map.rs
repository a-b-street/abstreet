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

extern crate map_model;

use map_model::{Map, RoadID, TurnID};
use road::GeomRoad;
use turn::GeomTurn;

pub struct GeomMap {
    pub roads: Vec<GeomRoad>,
    pub turns: Vec<GeomTurn>,
}

impl GeomMap {
    pub fn new(map: &Map) -> GeomMap {
        let bounds = map.get_gps_bounds();

        let mut roads: Vec<GeomRoad> = Vec::new();
        for r in map.all_roads() {
            roads.push(GeomRoad::new(r, &bounds));
        }

        let turns: Vec<GeomTurn> = map.all_turns()
            .iter()
            .map(|t| GeomTurn::new(&roads, t))
            .collect();

        GeomMap { roads, turns }
    }

    // The alt to these is implementing std::ops::Index, but that's way more verbose!
    pub fn get_r(&self, id: RoadID) -> &GeomRoad {
        &self.roads[id.0]
    }

    pub fn get_t(&self, id: TurnID) -> &GeomTurn {
        &self.turns[id.0]
    }
}

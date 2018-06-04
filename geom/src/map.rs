// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

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

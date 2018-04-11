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

use ezgui::input::UserInput;
use graphics::types::Color;
use map_model::{Map, Road, RoadID};
use piston::input::Key;
use render;
use std::collections::{HashSet, VecDeque};

// Keeps track of state so this can be interactively visualized
pub struct Floodfiller {
    visited: HashSet<RoadID>,
    // Order of expansion doesn't really matter, could use other things here
    queue: VecDeque<RoadID>,
}

impl Floodfiller {
    // TODO doesn't guarantee all visited roads are connected? are dead-ends possible with the
    // current turn definitions?
    pub fn new(start: RoadID) -> Floodfiller {
        let mut f = Floodfiller {
            visited: HashSet::new(),
            queue: VecDeque::new(),
        };
        f.queue.push_back(start);
        f
    }

    // TODO step backwards!

    pub fn step(&mut self, map: &Map) -> bool {
        loop {
            if self.queue.is_empty() {
                return true;
            }

            let r = map.get_r(self.queue.pop_front().unwrap());
            if self.visited.contains(&r.id) {
                continue;
            }
            self.visited.insert(r.id);
            for next in &map.get_next_roads(r.id) {
                if !self.visited.contains(&next.id) {
                    self.queue.push_back(next.id);
                }
            }

            return false;
        }
    }

    pub fn finish(&mut self, map: &Map) {
        loop {
            if self.step(map) {
                return;
            }
        }
    }

    // returns true if done
    pub fn event(&mut self, map: &Map, input: &mut UserInput) -> bool {
        if input.key_pressed(Key::Return, "Press Enter to quit floodfilling") {
            return true;
        }

        if !self.queue.is_empty() {
            if input.key_pressed(Key::Space, "Press space to step floodfilling forwards") {
                self.step(map);
            }
            if input.key_pressed(Key::Tab, "Press tab to floodfill the rest of the map") {
                self.finish(map);
            }
        }
        false
    }

    pub fn color_r(&self, r: &Road) -> Option<Color> {
        if self.visited.contains(&r.id) {
            return Some(render::VISITED_COLOR);
        }
        if !self.queue.is_empty() && *self.queue.front().unwrap() == r.id {
            return Some(render::NEXT_QUEUED_COLOR);
        }
        // TODO linear search shouldnt suck too much for interactive mode
        if self.queue.contains(&r.id) {
            return Some(render::QUEUED_COLOR);
        }
        None
    }
}

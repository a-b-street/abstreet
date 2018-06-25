// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::input::UserInput;
use graphics::types::Color;
use map_model::{Map, Road, RoadID};
use piston::input::Key;
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

    pub fn color_r(&self, r: &Road, cs: &ColorScheme) -> Option<Color> {
        if self.visited.contains(&r.id) {
            return Some(cs.get(Colors::Visited));
        }
        if !self.queue.is_empty() && *self.queue.front().unwrap() == r.id {
            return Some(cs.get(Colors::NextQueued));
        }
        // TODO linear search shouldnt suck too much for interactive mode
        if self.queue.contains(&r.id) {
            return Some(cs.get(Colors::Queued));
        }
        None
    }
}

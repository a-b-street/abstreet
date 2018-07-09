// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::input::UserInput;
use graphics::types::Color;
use map_model::{Map, Road, RoadID};
use piston::input::Key;
use std::collections::{HashSet, VecDeque};

// Keeps track of state so this can be interactively visualized
pub enum Floodfiller {
    Inactive,
    Active {
        visited: HashSet<RoadID>,
        // Order of expansion doesn't really matter, could use other things here
        queue: VecDeque<RoadID>,
    },
}

impl Floodfiller {
    pub fn new() -> Floodfiller {
        Floodfiller::Inactive
    }

    // TODO doesn't guarantee all visited roads are connected? are dead-ends possible with the
    // current turn definitions?
    pub fn start(start: RoadID) -> Floodfiller {
        let mut queue = VecDeque::new();
        queue.push_back(start);
        Floodfiller::Active {
            queue,
            visited: HashSet::new(),
        }
    }

    // TODO step backwards!

    // returns true if active
    pub fn event(&mut self, map: &Map, input: &mut UserInput) -> bool {
        let mut new_state: Option<Floodfiller> = None;
        let active = match self {
            Floodfiller::Inactive => false,
            Floodfiller::Active { visited, queue } => {
                if input.key_pressed(Key::Return, "Press Enter to quit floodfilling") {
                    new_state = Some(Floodfiller::Inactive);
                } else if !queue.is_empty() {
                    if input.key_pressed(Key::Space, "Press space to step floodfilling forwards") {
                        step(visited, queue, map);
                    }
                    if input.key_pressed(Key::Tab, "Press tab to floodfill the rest of the map") {
                        loop {
                            if step(visited, queue, map) {
                                break;
                            }
                        }
                    }
                }
                true
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        active
    }

    pub fn color_r(&self, r: &Road, cs: &ColorScheme) -> Option<Color> {
        match self {
            Floodfiller::Inactive => None,
            Floodfiller::Active { visited, queue } => {
                if visited.contains(&r.id) {
                    return Some(cs.get(Colors::Visited));
                }
                if !queue.is_empty() && *queue.front().unwrap() == r.id {
                    return Some(cs.get(Colors::NextQueued));
                }
                // TODO linear search shouldnt suck too much for interactive mode
                if queue.contains(&r.id) {
                    return Some(cs.get(Colors::Queued));
                }
                None
            }
        }
    }
}

fn step(visited: &mut HashSet<RoadID>, queue: &mut VecDeque<RoadID>, map: &Map) -> bool {
    loop {
        if queue.is_empty() {
            return true;
        }

        let r = map.get_r(queue.pop_front().unwrap());
        if visited.contains(&r.id) {
            continue;
        }
        visited.insert(r.id);
        for next in &map.get_next_roads(r.id) {
            if !visited.contains(&next.id) {
                queue.push_back(next.id);
            }
        }

        return false;
    }
}

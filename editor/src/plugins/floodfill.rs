// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::UserInput;
use graphics::types::Color;
use map_model::{Lane, LaneID, Map};
use piston::input::Key;
use std::collections::{HashSet, VecDeque};

// Keeps track of state so this can be interactively visualized
pub enum Floodfiller {
    Inactive,
    Active {
        visited: HashSet<LaneID>,
        // Order of expansion doesn't really matter, could use other things here
        queue: VecDeque<LaneID>,
    },
}

impl Floodfiller {
    pub fn new() -> Floodfiller {
        Floodfiller::Inactive
    }

    // TODO doesn't guarantee all visited lanes are connected? are dead-ends possible with the
    // current turn definitions?
    pub fn start(start: LaneID) -> Floodfiller {
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
                if input.key_pressed(Key::Return, "quit floodfilling") {
                    new_state = Some(Floodfiller::Inactive);
                } else if !queue.is_empty() {
                    if input.key_pressed(Key::Space, "step floodfilling forwards") {
                        step(visited, queue, map);
                    }
                    if input.key_pressed(Key::Tab, "floodfill the rest of the map") {
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

    pub fn color_l(&self, l: &Lane, cs: &ColorScheme) -> Option<Color> {
        match self {
            Floodfiller::Inactive => None,
            Floodfiller::Active { visited, queue } => {
                if visited.contains(&l.id) {
                    return Some(cs.get(Colors::Visited));
                }
                if !queue.is_empty() && *queue.front().unwrap() == l.id {
                    return Some(cs.get(Colors::NextQueued));
                }
                // TODO linear search shouldnt suck too much for interactive mode
                if queue.contains(&l.id) {
                    return Some(cs.get(Colors::Queued));
                }
                None
            }
        }
    }
}

fn step(visited: &mut HashSet<LaneID>, queue: &mut VecDeque<LaneID>, map: &Map) -> bool {
    loop {
        if queue.is_empty() {
            return true;
        }

        let l = map.get_l(queue.pop_front().unwrap());
        if visited.contains(&l.id) {
            continue;
        }
        visited.insert(l.id);
        for next in &map.get_next_lanes(l.id) {
            if !visited.contains(&next.id) {
                queue.push_back(next.id);
            }
        }

        return false;
    }
}

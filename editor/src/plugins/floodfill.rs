// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::Colors;
use ezgui::UserInput;
use graphics::types::Color;
use map_model::{LaneID, Map};
use objects::ID;
use piston::input::Key;
use plugins::{Colorizer, Ctx};
use std::collections::{HashSet, VecDeque};

// Keeps track of state so this can be interactively visualized
#[derive(PartialEq)]
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
    fn start(start: LaneID) -> Floodfiller {
        let mut queue = VecDeque::new();
        queue.push_back(start);
        Floodfiller::Active {
            queue,
            visited: HashSet::new(),
        }
    }

    // TODO step backwards!

    pub fn event(&mut self, map: &Map, input: &mut UserInput, selected: Option<ID>) -> bool {
        if *self == Floodfiller::Inactive {
            match selected {
                Some(ID::Lane(id)) => {
                    if input.key_pressed(Key::F, "start floodfilling from this lane") {
                        *self = Floodfiller::start(id);
                        return true;
                    }
                }
                _ => {}
            }
        }

        let mut new_state: Option<Floodfiller> = None;
        match self {
            Floodfiller::Inactive => {}
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
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            Floodfiller::Inactive => false,
            _ => true,
        }
    }
}

impl Colorizer for Floodfiller {
    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        match (self, obj) {
            (Floodfiller::Active { visited, queue }, ID::Lane(l)) => {
                if visited.contains(&l) {
                    return Some(ctx.cs.get(Colors::Visited));
                }
                if !queue.is_empty() && *queue.front().unwrap() == l {
                    return Some(ctx.cs.get(Colors::NextQueued));
                }
                // TODO linear search shouldnt suck too much for interactive mode
                if queue.contains(&l) {
                    return Some(ctx.cs.get(Colors::Queued));
                }
                None
            }
            _ => None,
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

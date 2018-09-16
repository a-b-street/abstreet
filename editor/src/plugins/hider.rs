// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use ezgui::UserInput;
use objects::ID;
use piston::input::Key;
use plugins::Colorizer;
use std::collections::HashSet;

pub struct Hider {
    items: HashSet<ID>,
}

impl Hider {
    pub fn new() -> Hider {
        Hider {
            items: HashSet::new(),
        }
    }

    pub fn event(&mut self, input: &mut UserInput, selected: &mut Option<ID>) -> bool {
        if input.unimportant_key_pressed(Key::K, "unhide everything") {
            println!("Unhiding {} things", self.items.len());
            self.items.clear();
            return true;
        }

        let item = match selected {
            // No real use case for hiding moving stuff
            Some(ID::Car(_)) => {
                return false;
            }
            Some(ID::Pedestrian(_)) => {
                return false;
            }
            Some(id) => id.clone(),
            _ => {
                return false;
            }
        };
        if input.unimportant_key_pressed(Key::H, &format!("hide {:?}", item)) {
            self.items.insert(item);
            println!("Hiding {:?}", item);
            *selected = None;
            return true;
        }
        false
    }

    pub fn show(&self, id: ID) -> bool {
        !self.items.contains(&id)
    }
}

impl Colorizer for Hider {}

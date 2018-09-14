// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use ezgui::UserInput;
use objects::ID;
use piston::input::Key;
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
            // TODO why not be able to hide anything?
            Some(id) => match id {
                ID::Intersection(_) => id.clone(),
                ID::Lane(_) => id.clone(),
                ID::Building(_) => id.clone(),
                ID::ExtraShape(_) => id.clone(),
                _ => {
                    return false;
                }
            },
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

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use ezgui::UserInput;
use kml::ExtraShapeID;
use map_model::{BuildingID, IntersectionID, LaneID};
use piston::input::Key;
use plugins::selection::{SelectionState, ID};
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

    pub fn event(&mut self, input: &mut UserInput, state: &mut SelectionState) -> bool {
        if input.unimportant_key_pressed(Key::K, "unhide everything") {
            println!("Unhiding {} things", self.items.len());
            self.items.clear();
            return true;
        }

        let item = match state {
            // TODO why not be able to hide anything?
            SelectionState::Selected(id) => match id.clone() {
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
            *state = SelectionState::Empty;
            return true;
        }
        false
    }

    pub fn show_l(&self, id: LaneID) -> bool {
        !self.items.contains(&ID::Lane(id))
    }

    pub fn show_b(&self, id: BuildingID) -> bool {
        !self.items.contains(&ID::Building(id))
    }

    pub fn show_i(&self, id: IntersectionID) -> bool {
        !self.items.contains(&ID::Intersection(id))
    }

    pub fn show_es(&self, id: ExtraShapeID) -> bool {
        !self.items.contains(&ID::ExtraShape(id))
    }
}

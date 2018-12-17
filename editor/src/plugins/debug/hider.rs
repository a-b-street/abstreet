use crate::objects::ID;
use ezgui::{Key, UserInput};
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

    pub fn show(&self, id: ID) -> bool {
        !self.items.contains(&id)
    }

    pub fn event(&mut self, input: &mut UserInput, selected: Option<ID>) -> bool {
        if input.action_chosen("unhide everything") {
            info!("Unhiding {} things", self.items.len());
            self.items.clear();
            return true;
        }

        let item = match selected {
            // No real use case for hiding moving stuff
            Some(ID::Car(_)) | Some(ID::Pedestrian(_)) | None => {
                return false;
            }
            Some(id) => id,
        };
        if input.contextual_action(Key::H, &format!("hide {:?}", item)) {
            self.items.insert(item);
            info!("Hiding {:?}", item);
            return true;
        }
        false
    }
}

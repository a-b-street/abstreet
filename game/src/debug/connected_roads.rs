use crate::helpers::ID;
use crate::ui::UI;
use ezgui::{EventCtx, Key};
use map_model::LaneID;
use std::collections::HashSet;

pub struct ShowConnectedRoads {
    key_held: bool,
    pub lanes: HashSet<LaneID>,
}

impl ShowConnectedRoads {
    pub fn new() -> ShowConnectedRoads {
        ShowConnectedRoads {
            key_held: false,
            lanes: HashSet::new(),
        }
    }

    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) {
        if self.key_held {
            self.key_held = !ctx.input.key_released(Key::RightAlt);
        } else {
            // TODO Can't really display an OSD action if we're not currently selecting something.
            // Could only activate sometimes, but that seems a bit harder to use.
            self.key_held = ctx.input.unimportant_key_pressed(
                Key::RightAlt,
                "hold right Alt to show roads connected to intersection",
            );
        }

        self.lanes.clear();
        if self.key_held {
            if let Some(ID::Intersection(i)) = ui.primary.current_selection {
                for r in &ui.primary.map.get_i(i).roads {
                    self.lanes.extend(ui.primary.map.get_r(*r).all_lanes());
                }
            }
        }
    }
}

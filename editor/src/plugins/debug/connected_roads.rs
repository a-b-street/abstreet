use crate::objects::{DrawCtx, ID};
use crate::plugins::{AmbientPlugin, PluginCtx};
use ezgui::{Color, Key};
use map_model::LaneID;
use std::collections::HashSet;

pub struct ShowConnectedRoads {
    key_held: bool,
    lanes: HashSet<LaneID>,
}

impl ShowConnectedRoads {
    pub fn new() -> ShowConnectedRoads {
        ShowConnectedRoads {
            key_held: false,
            lanes: HashSet::new(),
        }
    }
}

impl AmbientPlugin for ShowConnectedRoads {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
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
            if let Some(ID::Intersection(i)) = ctx.primary.current_selection {
                for r in &ctx.primary.map.get_i(i).roads {
                    self.lanes.extend(ctx.primary.map.get_r(*r).all_lanes());
                }
            }
        }
    }

    fn color_for(&self, obj: ID, ctx: &DrawCtx) -> Option<Color> {
        if let ID::Lane(id) = obj {
            if self.lanes.contains(&id) {
                return Some(ctx.cs.get("something associated with something else"));
            }
        }
        None
    }
}

use crate::objects::ID;
use crate::plugins::{NonblockingPlugin, PluginCtx};
use ezgui::Key;
use std::collections::HashSet;

pub struct Hider {
    items: HashSet<ID>,
}

impl Hider {
    pub fn new(ctx: &mut PluginCtx) -> Option<Hider> {
        if let Some(id) = hide_something(ctx) {
            let mut items = HashSet::new();
            items.insert(id);
            return Some(Hider { items });
        }
        None
    }

    pub fn show(&self, id: ID) -> bool {
        !self.items.contains(&id)
    }
}

impl NonblockingPlugin for Hider {
    fn nonblocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        // TODO Add non-prompt lines listing how much stuff is hidden. And if the numbers
        // align, "and a partridge in a pear tree..."
        ctx.input.set_mode("Object Hider", &ctx.canvas);

        if ctx.input.modal_action("unhide everything") {
            println!("Unhiding {} things", self.items.len());
            *ctx.recalculate_current_selection = true;
            ctx.primary.current_selection = None;
            return false;
        }

        if let Some(id) = hide_something(ctx) {
            self.items.insert(id);
        }
        true
    }
}

fn hide_something(ctx: &mut PluginCtx) -> Option<ID> {
    match ctx.primary.current_selection {
        // No real use case for hiding moving stuff
        Some(ID::Car(_)) | Some(ID::Pedestrian(_)) | None => None,
        // Can't hide stuff drawn in a batch
        Some(ID::Building(_)) | Some(ID::Road(_)) | Some(ID::Area(_)) | Some(ID::Parcel(_)) => None,
        Some(id) => {
            if ctx
                .input
                .contextual_action(Key::H, &format!("hide {:?}", id))
            {
                println!("Hiding {:?}", id);
                *ctx.recalculate_current_selection = true;
                ctx.primary.current_selection = None;
                Some(id)
            } else {
                None
            }
        }
    }
}

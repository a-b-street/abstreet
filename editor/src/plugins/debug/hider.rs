use crate::objects::ID;
use crate::plugins::PluginCtx;
use ezgui::Key;
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

    // Weird, true here means selection state changed.
    pub fn event(&mut self, ctx: &mut PluginCtx) -> bool {
        if !self.items.is_empty() {
            // TODO Add non-prompt lines listing how much stuff is hidden. And if the numbers
            // align, "and a partridge in a pear tree..."
            ctx.input.set_mode("Object Hider", &ctx.canvas);

            if ctx.input.modal_action("unhide everything") {
                info!("Unhiding {} things", self.items.len());
                self.items.clear();
                return true;
            }
        }

        let item = match ctx.primary.current_selection {
            // No real use case for hiding moving stuff
            Some(ID::Car(_)) | Some(ID::Pedestrian(_)) | None => {
                return false;
            }
            Some(id) => id,
        };
        if ctx
            .input
            .contextual_action(Key::H, &format!("hide {:?}", item))
        {
            self.items.insert(item);
            info!("Hiding {:?}", item);
            return true;
        }
        false
    }
}

pub mod chokepoints;
pub mod classification;
pub mod floodfill;
pub mod geom_validation;
pub mod hider;
pub mod layers;

use crate::objects::ID;
use crate::plugins::{Plugin, PluginCtx};

pub struct DebugMode {
    // Ambient; they don't conflict with any of the main plugins.
    hider: hider::Hider,
    pub layers: layers::ToggleableLayers,
}

impl DebugMode {
    pub fn new() -> DebugMode {
        DebugMode {
            hider: hider::Hider::new(),
            layers: layers::ToggleableLayers::new(),
        }
    }

    pub fn show(&self, obj: ID) -> bool {
        self.hider.show(obj) && self.layers.show(obj)
    }
}

impl Plugin for DebugMode {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        // Always run ambient plugins. If either returns true, the selection state could have
        // changed.
        if self.hider.event(ctx) || self.layers.event(ctx.input) {
            *ctx.recalculate_current_selection = true;
            ctx.primary.current_selection = None;
        }
        false
    }
}

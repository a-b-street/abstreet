mod chokepoints;
mod classification;
mod floodfill;
mod geom_validation;
mod hider;
mod layers;

use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx};

pub struct DebugMode {
    active_plugin: Option<Box<Plugin>>,

    // Ambient; they don't conflict with any of the main plugins.
    hider: hider::Hider,
    pub layers: layers::ToggleableLayers,
}

impl DebugMode {
    pub fn new() -> DebugMode {
        DebugMode {
            active_plugin: None,
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

        if self.active_plugin.is_some() {
            if self.active_plugin.as_mut().unwrap().blocking_event(ctx) {
                return true;
            } else {
                self.active_plugin = None;
                return false;
            }
        }

        if let Some(p) = chokepoints::ChokepointsFinder::new(ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = classification::OsmClassifier::new(ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = floodfill::Floodfiller::new(ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = geom_validation::Validator::new(ctx) {
            self.active_plugin = Some(Box::new(p));
        }

        self.active_plugin.is_some()
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let Some(ref plugin) = self.active_plugin {
            plugin.draw(g, ctx);
        }
    }

    fn color_for(&self, obj: ID, ctx: &Ctx) -> Option<Color> {
        if let Some(ref plugin) = self.active_plugin {
            return plugin.color_for(obj, ctx);
        }
        None
    }
}

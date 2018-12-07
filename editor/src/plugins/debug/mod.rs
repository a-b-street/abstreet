mod chokepoints;
mod classification;
mod floodfill;
mod geom_validation;
mod hider;
pub mod layers;
mod steep;

use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{Color, GfxCtx};
use map_model::Map;

pub struct DebugMode {
    // steepness acts like one of the active plugins, except that it needs to cache state while
    // inactive.
    active_plugin: Option<Box<Plugin>>,
    steepness: steep::SteepnessVisualizer,

    // Ambient; they don't conflict with any of the main plugins.
    hider: hider::Hider,
    pub layers: layers::ToggleableLayers,
}

impl DebugMode {
    pub fn new(map: &Map) -> DebugMode {
        DebugMode {
            active_plugin: None,
            steepness: steep::SteepnessVisualizer::new(map),
            hider: hider::Hider::new(),
            layers: layers::ToggleableLayers::new(),
        }
    }

    pub fn show(&self, obj: ID) -> bool {
        self.hider.show(obj) && self.layers.show(obj)
    }
}

impl Plugin for DebugMode {
    fn event(&mut self, mut ctx: PluginCtx) -> bool {
        // Always run ambient plugins. If either returns true, the selection state could have
        // changed.
        if self
            .hider
            .event(&mut ctx.input, ctx.primary.current_selection)
            || self.layers.event(&mut ctx.input)
        {
            ctx.primary.recalculate_current_selection = true;
            ctx.primary.current_selection = None;
        }

        if self.active_plugin.is_some() {
            if self.active_plugin.as_mut().unwrap().new_event(&mut ctx) {
                return true;
            } else {
                self.active_plugin = None;
                return false;
            }
        } else if self.steepness.active {
            return self.steepness.new_event(&mut ctx);
        }

        if let Some(p) = chokepoints::ChokepointsFinder::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = classification::OsmClassifier::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = floodfill::Floodfiller::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = geom_validation::Validator::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if self.steepness.new_event(&mut ctx) {
            return true;
        }

        self.active_plugin.is_some()
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &mut Ctx) {
        if let Some(ref plugin) = self.active_plugin {
            plugin.draw(g, ctx);
        } else if self.steepness.active {
            self.steepness.draw(g, ctx);
        }
    }

    fn color_for(&self, obj: ID, ctx: &mut Ctx) -> Option<Color> {
        if let Some(ref plugin) = self.active_plugin {
            return plugin.color_for(obj, ctx);
        } else if self.steepness.active {
            return self.steepness.color_for(obj, ctx);
        }
        None
    }
}

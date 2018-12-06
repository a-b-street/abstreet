use ezgui::{Color, GfxCtx};
use objects::{Ctx, ID};
use plugins;
use plugins::{Plugin, PluginCtx};

pub struct DebugMode {
    active_plugin: Option<Box<Plugin>>,
}

impl DebugMode {
    pub fn new() -> DebugMode {
        DebugMode {
            active_plugin: None,
        }
    }
}

impl Plugin for DebugMode {
    fn event(&mut self, mut ctx: PluginCtx) -> bool {
        if self.active_plugin.is_some() {
            if self.active_plugin.as_mut().unwrap().new_event(&mut ctx) {
                return true;
            } else {
                self.active_plugin = None;
                return false;
            }
        }

        if let Some(p) = plugins::debug::chokepoints::ChokepointsFinder::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = plugins::debug::classification::OsmClassifier::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = plugins::debug::floodfill::Floodfiller::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        } else if let Some(p) = plugins::debug::geom_validation::Validator::new(&mut ctx) {
            self.active_plugin = Some(Box::new(p));
        }

        self.active_plugin.is_some()
    }

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        if let Some(ref plugin) = self.active_plugin {
            plugin.draw(g, ctx);
        }
    }

    fn color_for(&self, obj: ID, ctx: Ctx) -> Option<Color> {
        if let Some(ref plugin) = self.active_plugin {
            return plugin.color_for(obj, ctx);
        }
        None
    }
}

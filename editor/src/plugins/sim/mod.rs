mod controls;
mod diff_all;
mod diff_trip;
mod show_score;

use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::GfxCtx;
use piston::input::Key;

// TODO This is per UI, so it's never reloaded. Make sure to detect new loads, even when the
// initial time is 0? But we probably have no state then, so...
pub struct SimMode {
    // These can't coexist.
    diff_plugin: Option<Box<Plugin>>,
    ambient_plugins: Vec<Box<Plugin>>,
}

impl SimMode {
    pub fn new() -> SimMode {
        SimMode {
            diff_plugin: None,
            ambient_plugins: vec![
                Box::new(show_score::ShowScoreState::new(Key::Period)),
                Box::new(controls::SimControls::new()),
            ],
        }
    }
}

impl Plugin for SimMode {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        if self.diff_plugin.is_some() {
            if self.diff_plugin.as_mut().unwrap().blocking_event(ctx) {
                return true;
            } else {
                self.diff_plugin = None;
                return false;
            }
        }

        if ctx.secondary.is_some() {
            if let Some(p) = diff_all::DiffAllState::new(Key::D, ctx) {
                self.diff_plugin = Some(Box::new(p));
            } else if let Some(p) = diff_trip::DiffTripState::new(Key::B, ctx) {
                self.diff_plugin = Some(Box::new(p));
            }
        }

        for p in self.ambient_plugins.iter_mut() {
            p.ambient_event(ctx);
        }

        // TODO Should the diff plugins block other stuff?
        self.diff_plugin.is_some()
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let Some(ref plugin) = self.diff_plugin {
            plugin.draw(g, ctx);
        }

        for p in &self.ambient_plugins {
            p.draw(g, ctx);
        }
    }

    // Nothing in SimMode implements color_for.
}

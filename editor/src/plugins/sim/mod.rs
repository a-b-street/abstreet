mod controls;
mod diff_all;
mod diff_trip;
mod show_score;

use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::GfxCtx;
use sim::{Event, Sim, Tick};

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
                // TODO Could be a little simpler to instantiate this lazily, stop representing
                // inactive state.
                Box::new(show_score::ShowScoreState::new()),
                Box::new(controls::SimControls::new()),
            ],
        }
    }

    pub fn get_new_primary_events(
        &self,
        last_seen_tick: Option<Tick>,
    ) -> Option<(Tick, &Vec<Event>)> {
        let (tick, events) = self.ambient_plugins[1]
            .downcast_ref::<controls::SimControls>()
            .unwrap()
            .primary_events
            .as_ref()?;
        if last_seen_tick.is_none() || last_seen_tick != Some(*tick) {
            Some((*tick, events))
        } else {
            None
        }
    }

    pub fn run_sim(&mut self, primary_sim: &mut Sim) {
        self.ambient_plugins[1]
            .downcast_mut::<controls::SimControls>()
            .unwrap()
            .run_sim(primary_sim);
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
            if let Some(p) = diff_all::DiffAllState::new(ctx) {
                self.diff_plugin = Some(Box::new(p));
            } else if let Some(p) = diff_trip::DiffTripState::new(ctx) {
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

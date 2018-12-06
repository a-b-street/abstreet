use ezgui::{Color, GfxCtx};
use objects::{Ctx, ID};
use plugins;
use plugins::{Plugin, PluginCtx};

pub struct ViewMode {
    ambient_plugins: Vec<Box<Plugin>>,
}

impl ViewMode {
    pub fn new() -> ViewMode {
        ViewMode {
            ambient_plugins: vec![
                Box::new(plugins::view::follow::FollowState::new()),
                Box::new(plugins::view::debug_objects::DebugObjectsState::new()),
                Box::new(plugins::view::show_activity::ShowActivityState::new()),
                Box::new(plugins::view::show_owner::ShowOwnerState::new()),
                Box::new(plugins::view::show_route::ShowRouteState::new()),
                Box::new(plugins::view::turn_cycler::TurnCyclerState::new()),
            ],
        }
    }
}

impl Plugin for ViewMode {
    fn event(&mut self, mut ctx: PluginCtx) -> bool {
        for p in self.ambient_plugins.iter_mut() {
            p.ambient_event(&mut ctx);
        }
        false
    }

    fn draw(&self, g: &mut GfxCtx, mut ctx: Ctx) {
        for p in &self.ambient_plugins {
            p.new_draw(g, &mut ctx);
        }
    }

    fn color_for(&self, obj: ID, mut ctx: Ctx) -> Option<Color> {
        // First one arbitrarily wins.
        // TODO Maybe none of these actually do this?
        for p in &self.ambient_plugins {
            if let Some(c) = p.new_color_for(obj, &mut ctx) {
                return Some(c);
            }
        }
        None
    }
}

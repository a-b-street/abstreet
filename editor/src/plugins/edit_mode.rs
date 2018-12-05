use ezgui::{Color, GfxCtx};
use map_model::IntersectionID;
use objects::{Ctx, ID};
use plugins;
use plugins::edit::stop_sign_editor::StopSignEditor;
use plugins::edit::traffic_signal_editor::TrafficSignalEditor;
use plugins::{Plugin, PluginCtx};

pub struct EditMode {
    active_plugin: Option<Box<Plugin>>,
}

impl EditMode {
    pub fn new() -> EditMode {
        EditMode {
            active_plugin: None,
        }
    }

    pub fn show_turn_icons(&self, id: IntersectionID) -> bool {
        if let Some(p) = self
            .active_plugin
            .as_ref()
            .and_then(|p| p.downcast_ref::<StopSignEditor>().ok())
        {
            return p.show_turn_icons(id);
        }
        if let Some(p) = self
            .active_plugin
            .as_ref()
            .and_then(|p| p.downcast_ref::<TrafficSignalEditor>().ok())
        {
            return p.show_turn_icons(id);
        }
        false
    }
}

impl Plugin for EditMode {
    fn event(&mut self, mut ctx: PluginCtx) -> bool {
        if self.active_plugin.is_some() {
            if self.active_plugin.as_mut().unwrap().new_event(&mut ctx) {
                return true;
            } else {
                self.active_plugin = None;
                return false;
            }
        }

        // TODO Something higher-level should not even invoke EditMode while we're in A/B test
        // mode.
        if ctx.secondary.is_some() {
            return false;
        }

        // TODO Make better constructors
        {
            let mut x = plugins::edit::a_b_tests::ABTestManager::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }
        {
            let mut x = plugins::edit::color_picker::ColorPicker::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }
        {
            let mut x = plugins::edit::draw_neighborhoods::DrawNeighborhoodState::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }
        {
            let mut x = plugins::edit::map_edits::EditsManager::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }
        {
            let mut x = plugins::edit::road_editor::RoadEditor::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }
        {
            let mut x = plugins::edit::scenarios::ScenarioManager::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }
        {
            let mut x = StopSignEditor::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }
        {
            let mut x = TrafficSignalEditor::new();
            if x.new_event(&mut ctx) {
                self.active_plugin = Some(Box::new(x));
                return true;
            }
        }

        false
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

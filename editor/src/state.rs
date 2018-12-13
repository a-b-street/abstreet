use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::plugins::debug::layers::ToggleableLayers;
use crate::plugins::debug::DebugMode;
use crate::plugins::edit::EditMode;
use crate::plugins::logs::DisplayLogs;
use crate::plugins::sim::SimMode;
use crate::plugins::time_travel::TimeTravel;
use crate::plugins::tutorial::TutorialMode;
use crate::plugins::view::ViewMode;
use crate::plugins::{Plugin, PluginCtx};
use crate::render::Renderable;
use crate::ui::PerMapUI;
use abstutil::Timer;
use ezgui::{Canvas, Color, GfxCtx, UserInput};
use map_model::IntersectionID;
use sim::{GetDrawAgents, SimFlags};

pub trait UIState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64);
    fn set_current_selection(&mut self, obj: Option<ID>);
    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    );
    fn get_objects_onscreen(
        &self,
        canvas: &Canvas,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>);
    fn is_debug_mode_enabled(&self) -> bool;
    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx);
    fn dump_before_abort(&self);
    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color>;
    fn primary(&self) -> &PerMapUI;
}

pub struct DefaultUIState {
    primary: PerMapUI,
    primary_plugins: PluginsPerMap,
    // When running an A/B test, this is populated too.
    secondary: Option<(PerMapUI, PluginsPerMap)>,
    plugins: PluginsPerUI,
    active_plugin: Option<usize>,
}

impl DefaultUIState {
    pub fn new(flags: SimFlags, kml: Option<String>, canvas: &Canvas) -> DefaultUIState {
        // Do this first to trigger the log console initialization, so anything logged by sim::load
        // isn't lost.
        let plugins = PluginsPerUI::new(&flags);

        let (primary, primary_plugins) = PerMapUI::new(flags, kml, &canvas);
        DefaultUIState {
            primary,
            primary_plugins,
            secondary: None,
            plugins,
            active_plugin: None,
        }
    }

    fn get_active_plugin(&self) -> Option<&Box<Plugin>> {
        let idx = self.active_plugin?;
        let len = self.plugins.list.len();
        if idx < len {
            Some(&self.plugins.list[idx])
        } else {
            Some(&self.primary_plugins.list[idx - len])
        }
    }

    fn run_plugin(
        &mut self,
        idx: usize,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    ) -> bool {
        let mut ctx = PluginCtx {
            primary: &mut self.primary,
            primary_plugins: None,
            secondary: &mut self.secondary,
            canvas,
            cs,
            input,
            hints,
            recalculate_current_selection,
        };
        let len = self.plugins.list.len();
        if idx < len {
            ctx.primary_plugins = Some(&mut self.primary_plugins);
            self.plugins.list[idx].blocking_event(&mut ctx)
        } else {
            self.primary_plugins.list[idx - len].blocking_event(&mut ctx)
        }
    }
}

impl UIState for DefaultUIState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64) {
        self.primary_plugins
            .layers_mut()
            .handle_zoom(old_zoom, new_zoom);
    }

    fn set_current_selection(&mut self, obj: Option<ID>) {
        self.primary.current_selection = obj;
    }

    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    ) {
        // If there's an active plugin, just run it.
        if let Some(idx) = self.active_plugin {
            if !self.run_plugin(idx, input, hints, recalculate_current_selection, cs, canvas) {
                self.active_plugin = None;
            }
        } else {
            // Run each plugin, short-circuiting if the plugin claimed it was active.
            for idx in 0..self.plugins.list.len() + self.primary_plugins.list.len() {
                if self.run_plugin(idx, input, hints, recalculate_current_selection, cs, canvas) {
                    self.active_plugin = Some(idx);
                    break;
                }
            }
        }
    }

    fn get_objects_onscreen(
        &self,
        canvas: &Canvas,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>) {
        let draw_agent_source: &GetDrawAgents = {
            let tt = self.primary_plugins.time_travel();
            if tt.is_active() {
                tt
            } else {
                &self.primary.sim
            }
        };

        self.primary.draw_map.get_objects_onscreen(
            canvas.get_screen_bounds(),
            self.primary_plugins.debug_mode(),
            &self.primary.map,
            draw_agent_source,
            self,
        )
    }

    fn is_debug_mode_enabled(&self) -> bool {
        self.primary_plugins.layers().debug_mode.is_enabled()
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let Some(p) = self.get_active_plugin() {
            p.draw(g, ctx);
        } else {
            // If no other mode was active, give the ambient plugins in ViewMode and SimMode a
            // chance.
            self.primary_plugins.view_mode().draw(g, ctx);
            self.plugins.sim_mode().draw(g, ctx);
        }
    }

    fn dump_before_abort(&self) {
        error!("********************************************************************************");
        error!("UI broke! Primary sim:");
        self.primary.sim.dump_before_abort();
        if let Some((s, _)) = &self.secondary {
            error!("Secondary sim:");
            s.sim.dump_before_abort();
        }
    }

    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color> {
        if Some(id) == self.primary.current_selection {
            return Some(ctx.cs.get_def("selected", Color::BLUE));
        }

        if let Some(p) = self.get_active_plugin() {
            p.color_for(id, ctx)
        } else {
            // If no other mode was active, give the ambient plugins in ViewMode a chance.
            self.primary_plugins.view_mode().color_for(id, ctx)
        }
    }

    fn primary(&self) -> &PerMapUI {
        &self.primary
    }
}

pub trait ShowTurnIcons {
    fn show_icons_for(&self, id: IntersectionID) -> bool;
}

impl ShowTurnIcons for DefaultUIState {
    fn show_icons_for(&self, id: IntersectionID) -> bool {
        self.primary_plugins
            .layers()
            .show_all_turn_icons
            .is_enabled()
            || self.plugins.edit_mode().show_turn_icons(id)
            || {
                if let Some(ID::Turn(t)) = self.primary.current_selection {
                    t.parent == id
                } else {
                    false
                }
            }
    }
}

// aka plugins that don't depend on map
pub struct PluginsPerUI {
    pub list: Vec<Box<Plugin>>,
}

impl PluginsPerUI {
    pub fn new(flags: &SimFlags) -> PluginsPerUI {
        let mut plugins = PluginsPerUI {
            list: vec![
                Box::new(EditMode::new()),
                Box::new(SimMode::new()),
                Box::new(DisplayLogs::new()),
            ],
        };

        // TODO Hacktastic way of sneaking this in!
        if flags.load == "../data/raw_maps/ban_left_turn.abst".to_string() {
            plugins.list.push(Box::new(TutorialMode::new()));
        }

        plugins
    }

    pub fn edit_mode(&self) -> &EditMode {
        self.list[0].downcast_ref::<EditMode>().unwrap()
    }

    pub fn sim_mode(&self) -> &Box<Plugin> {
        &self.list[1]
    }
}

pub struct PluginsPerMap {
    // Anything that holds onto any kind of ID has to live here!
    pub list: Vec<Box<Plugin>>,
}

impl PluginsPerMap {
    pub fn new(state: &PerMapUI, canvas: &Canvas, timer: &mut Timer) -> PluginsPerMap {
        let mut plugins = PluginsPerMap {
            list: vec![
                Box::new(DebugMode::new(&state.map)),
                Box::new(ViewMode::new(&state.map, &state.draw_map, timer)),
                Box::new(TimeTravel::new()),
            ],
        };
        plugins.layers_mut().handle_zoom(-1.0, canvas.cam_zoom);
        plugins
    }

    pub fn debug_mode(&self) -> &DebugMode {
        self.list[0].downcast_ref::<DebugMode>().unwrap()
    }

    pub fn view_mode(&self) -> &Box<Plugin> {
        &self.list[1]
    }

    pub fn time_travel(&self) -> &TimeTravel {
        self.list[2].downcast_ref::<TimeTravel>().unwrap()
    }

    pub fn layers(&self) -> &ToggleableLayers {
        &self.list[0].downcast_ref::<DebugMode>().unwrap().layers
    }

    pub fn layers_mut(&mut self) -> &mut ToggleableLayers {
        &mut self.list[0].downcast_mut::<DebugMode>().unwrap().layers
    }
}

use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::plugins::debug::DebugMode;
use crate::plugins::edit::EditMode;
use crate::plugins::logs::DisplayLogs;
use crate::plugins::sim::SimMode;
use crate::plugins::time_travel::TimeTravel;
use crate::plugins::view::ViewMode;
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{DrawMap, Renderable};
use abstutil::Timer;
use ezgui::{Canvas, Color, GfxCtx, UserInput};
use map_model::{IntersectionID, Map};
use sim::{GetDrawAgents, Sim, SimFlags, Tick};

pub trait UIState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64);
    fn set_current_selection(&mut self, obj: Option<ID>);
    fn is_current_selection(&self, obj: ID) -> bool;
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
    pub primary: PerMapUI,
    primary_plugins: PluginsPerMap,
    // When running an A/B test, this is populated too.
    secondary: Option<(PerMapUI, PluginsPerMap)>,

    edit_mode: EditMode,
    pub sim_mode: SimMode,
    logs: DisplayLogs,

    active_plugin: Option<usize>,
}

impl DefaultUIState {
    pub fn new(flags: SimFlags, kml: Option<String>, canvas: &Canvas) -> DefaultUIState {
        // Do this first to trigger the log console initialization, so anything logged by sim::load
        // isn't lost.
        let logs = DisplayLogs::new();
        let (primary, primary_plugins) = PerMapUI::new(flags, kml, &canvas);
        DefaultUIState {
            primary,
            primary_plugins,
            secondary: None,
            edit_mode: EditMode::new(),
            sim_mode: SimMode::new(),
            logs,
            active_plugin: None,
        }
    }

    fn get_active_plugin(&self) -> Option<&Plugin> {
        let idx = self.active_plugin?;
        match idx {
            x if x == 0 => Some(&self.edit_mode),
            x if x == 1 => Some(&self.sim_mode),
            x if x == 2 => Some(&self.logs),
            x if x == 3 => Some(&self.primary_plugins.debug_mode),
            x if x == 4 => Some(&self.primary_plugins.view_mode),
            x if x == 5 => Some(&self.primary_plugins.time_travel),
            _ => {
                panic!("Illegal active_plugin {}", idx);
            }
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
        match idx {
            x if x == 0 => {
                ctx.primary_plugins = Some(&mut self.primary_plugins);
                self.edit_mode.blocking_event(&mut ctx)
            }
            x if x == 1 => {
                ctx.primary_plugins = Some(&mut self.primary_plugins);
                self.sim_mode.blocking_event(&mut ctx)
            }
            x if x == 2 => self.logs.blocking_event(&mut ctx),
            x if x == 3 => self.primary_plugins.debug_mode.blocking_event(&mut ctx),
            x if x == 4 => self.primary_plugins.view_mode.blocking_event(&mut ctx),
            x if x == 5 => self.primary_plugins.time_travel.blocking_event(&mut ctx),
            _ => {
                panic!("Illegal active_plugin {}", idx);
            }
        }
    }
}

impl UIState for DefaultUIState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64) {
        self.primary_plugins
            .debug_mode
            .layers
            .handle_zoom(old_zoom, new_zoom);
    }

    fn set_current_selection(&mut self, obj: Option<ID>) {
        self.primary.current_selection = obj;
    }

    fn is_current_selection(&self, obj: ID) -> bool {
        self.primary.current_selection == Some(obj)
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
            for idx in 0..=5 {
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
            let tt = &self.primary_plugins.time_travel;
            if tt.is_active() {
                tt
            } else {
                &self.primary.sim
            }
        };

        self.primary.draw_map.get_objects_onscreen(
            canvas.get_screen_bounds(),
            &self.primary_plugins.debug_mode,
            &self.primary.map,
            draw_agent_source,
            self,
        )
    }

    fn is_debug_mode_enabled(&self) -> bool {
        self.primary_plugins
            .debug_mode
            .layers
            .debug_mode
            .is_enabled()
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let Some(p) = self.get_active_plugin() {
            p.draw(g, ctx);
        } else {
            // If no other mode was active, give the ambient plugins in ViewMode and SimMode a
            // chance.
            self.primary_plugins.view_mode.draw(g, ctx);
            self.sim_mode.draw(g, ctx);
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
        match id {
            ID::Turn(_) => {}
            _ => {
                if Some(id) == self.primary.current_selection {
                    return Some(ctx.cs.get_def("selected", Color::BLUE));
                }
            }
        };

        if let Some(p) = self.get_active_plugin() {
            p.color_for(id, ctx)
        } else {
            // If no other mode was active, give the ambient plugins in ViewMode a chance.
            self.primary_plugins.view_mode.color_for(id, ctx)
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
            .debug_mode
            .layers
            .show_all_turn_icons
            .is_enabled()
            || self.edit_mode.show_turn_icons(id)
            || {
                if let Some(ID::Turn(t)) = self.primary.current_selection {
                    t.parent == id
                } else {
                    false
                }
            }
    }
}

// All of the state that's bound to a specific map+edit has to live here.
pub struct PerMapUI {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub current_flags: SimFlags,
}

impl PerMapUI {
    pub fn new(flags: SimFlags, kml: Option<String>, canvas: &Canvas) -> (PerMapUI, PluginsPerMap) {
        let mut timer = abstutil::Timer::new("setup PerMapUI");

        let (map, sim) = sim::load(flags.clone(), Some(Tick::from_seconds(30)), &mut timer);
        let extra_shapes: Vec<kml::ExtraShape> = if let Some(path) = kml {
            if path.ends_with(".kml") {
                kml::load(&path, &map.get_gps_bounds(), &mut timer)
                    .expect("Couldn't load extra KML shapes")
                    .shapes
            } else {
                let shapes: kml::ExtraShapes =
                    abstutil::read_binary(&path, &mut timer).expect("Couldn't load ExtraShapes");
                shapes.shapes
            }
        } else {
            Vec::new()
        };

        timer.start("draw_map");
        let draw_map = DrawMap::new(&map, extra_shapes, &mut timer);
        timer.stop("draw_map");

        let state = PerMapUI {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags,
        };
        let plugins = PluginsPerMap::new(&state, canvas, &mut timer);
        timer.done();
        (state, plugins)
    }
}

pub struct PluginsPerMap {
    // Anything that holds onto any kind of ID has to live here!
    debug_mode: DebugMode,
    view_mode: ViewMode,
    time_travel: TimeTravel,
}

impl PluginsPerMap {
    pub fn new(state: &PerMapUI, canvas: &Canvas, timer: &mut Timer) -> PluginsPerMap {
        let mut plugins = PluginsPerMap {
            debug_mode: DebugMode::new(&state.map),
            view_mode: ViewMode::new(&state.map, &state.draw_map, timer),
            time_travel: TimeTravel::new(),
        };
        plugins.debug_mode.layers.handle_zoom(-1.0, canvas.cam_zoom);
        plugins
    }
}

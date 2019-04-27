use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, RenderingHints, ID};
use crate::plugins;
use crate::plugins::{edit, view, AmbientPlugin, BlockingPlugin, NonblockingPlugin, PluginCtx};
use crate::render::DrawMap;
use abstutil::{MeasureMemory, Timer};
use ezgui::EventCtx;
use ezgui::{Color, GfxCtx, Prerender};
use geom::Duration;
use map_model::Map;
use sim::{Sim, SimFlags};
use structopt::StructOpt;

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "editor")]
pub struct Flags {
    #[structopt(flatten)]
    pub sim_flags: SimFlags,

    /// Extra KML or ExtraShapes to display
    #[structopt(long = "kml")]
    pub kml: Option<String>,

    // TODO Ideally these'd be phrased positively, but can't easily make them default to true.
    /// Should lane markings be drawn? Sometimes they eat too much GPU memory.
    #[structopt(long = "dont_draw_lane_markings")]
    pub dont_draw_lane_markings: bool,

    /// Enable cpuprofiler?
    #[structopt(long = "enable_profiler")]
    pub enable_profiler: bool,

    /// Number of agents to generate when small_spawn called
    #[structopt(long = "num_agents", default_value = "100")]
    pub num_agents: usize,

    /// Don't start with the splash screen and menu
    #[structopt(long = "no_splash")]
    pub no_splash: bool,
}

pub struct UIState {
    pub primary: PerMapUI,
    pub primary_plugins: PluginsPerMap,
    // When running an A/B test, this is populated too.
    pub secondary: Option<(PerMapUI, PluginsPerMap)>,

    // These are all mutually exclusive and, if present, override everything else.
    pub exclusive_blocking_plugin: Option<Box<BlockingPlugin>>,
    // These are all mutually exclusive, but don't override other stuff.
    exclusive_nonblocking_plugin: Option<Box<NonblockingPlugin>>,

    pub enable_debug_controls: bool,

    pub cs: ColorScheme,
}

impl UIState {
    pub fn new(flags: Flags, prerender: &Prerender, enable_debug_controls: bool) -> UIState {
        let cs = ColorScheme::load().unwrap();

        let (primary, primary_plugins) = PerMapUI::new(flags, &cs, prerender);
        UIState {
            primary,
            primary_plugins,
            secondary: None,
            exclusive_blocking_plugin: None,
            exclusive_nonblocking_plugin: None,
            enable_debug_controls,
            cs,
        }
    }

    pub fn color_obj(&self, id: ID, ctx: &DrawCtx) -> Option<Color> {
        if let Some(ref plugin) = self.exclusive_blocking_plugin {
            return plugin.color_for(id, ctx);
        }

        // The exclusive_nonblocking_plugins don't color_obj.

        for p in &self.primary_plugins.ambient_plugins {
            if let Some(c) = p.color_for(id, ctx) {
                return Some(c);
            }
        }

        None
    }

    pub fn event(
        &mut self,
        event_ctx: &mut EventCtx,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
    ) {
        let mut ctx = PluginCtx {
            primary: &mut self.primary,
            secondary: &mut self.secondary,
            canvas: event_ctx.canvas,
            cs: &mut self.cs,
            prerender: event_ctx.prerender,
            input: event_ctx.input,
            hints,
            recalculate_current_selection,
        };

        // Exclusive blocking plugins first
        {
            if self.exclusive_blocking_plugin.is_some() {
                if !self
                    .exclusive_blocking_plugin
                    .as_mut()
                    .unwrap()
                    .blocking_event_with_plugins(&mut ctx, &mut self.primary_plugins)
                {
                    self.exclusive_blocking_plugin = None;
                }
                return;
            }

            if let Some(p) = view::warp::WarpState::new(&mut ctx) {
                self.exclusive_blocking_plugin = Some(Box::new(p));
            } else if ctx.secondary.is_none() {
                if let Some(p) = edit::a_b_tests::ABTestManager::new(&mut ctx) {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                }
            }
        }

        // Exclusive nonblocking plugins
        {
            if self.exclusive_nonblocking_plugin.is_some() {
                if !self
                    .exclusive_nonblocking_plugin
                    .as_mut()
                    .unwrap()
                    .nonblocking_event(&mut ctx)
                {
                    self.exclusive_nonblocking_plugin = None;
                }
            } else if ctx.secondary.is_some() {
                // TODO This is per UI, so it's never reloaded. Make sure to detect new loads, even
                // when the initial time is 0? But we probably have no state then, so...
                if let Some(p) = plugins::sim::diff_all::DiffAllState::new(&mut ctx) {
                    self.exclusive_nonblocking_plugin = Some(Box::new(p));
                } else if let Some(p) = plugins::sim::diff_trip::DiffTripState::new(&mut ctx) {
                    self.exclusive_nonblocking_plugin = Some(Box::new(p));
                }
            }
        }

        // Ambient plugins
        for p in self.primary_plugins.ambient_plugins.iter_mut() {
            p.ambient_event(&mut ctx);
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if let Some(ref plugin) = self.exclusive_blocking_plugin {
            plugin.draw(g, ctx);
            return;
        }

        if let Some(ref plugin) = self.exclusive_nonblocking_plugin {
            plugin.draw(g, ctx);
        }

        // Stackable modals
        for p in &self.primary_plugins.ambient_plugins {
            p.draw(g, ctx);
        }
    }
}

// All of the state that's bound to a specific map+edit has to live here.
pub struct PerMapUI {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub current_flags: Flags,
}

impl PerMapUI {
    pub fn new(flags: Flags, cs: &ColorScheme, prerender: &Prerender) -> (PerMapUI, PluginsPerMap) {
        let mut timer = abstutil::Timer::new("setup PerMapUI");
        let mut mem = MeasureMemory::new();
        let (map, sim, _) = flags
            .sim_flags
            .load(Some(Duration::seconds(30.0)), &mut timer);
        mem.reset("Map and Sim", &mut timer);

        timer.start("draw_map");
        let draw_map = DrawMap::new(&map, &flags, cs, prerender, &mut timer);
        timer.stop("draw_map");
        mem.reset("DrawMap", &mut timer);

        let state = PerMapUI {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags.clone(),
        };
        let plugins = PluginsPerMap::new(&state, prerender, &mut timer);
        (state, plugins)
    }
}

// Anything that holds onto any kind of ID has to live here!
pub struct PluginsPerMap {
    ambient_plugins: Vec<Box<AmbientPlugin>>,
}

impl PluginsPerMap {
    pub fn new(state: &PerMapUI, prerender: &Prerender, timer: &mut Timer) -> PluginsPerMap {
        PluginsPerMap {
            ambient_plugins: vec![
                Box::new(view::neighborhood_summary::NeighborhoodSummary::new(
                    &state.map,
                    &state.draw_map,
                    prerender,
                    timer,
                )),
                // TODO Could be a little simpler to instantiate this lazily, stop representing
                // inactive state.
                Box::new(view::show_associated::ShowAssociatedState::new()),
                Box::new(view::turn_cycler::TurnCyclerState::new()),
            ],
        }
    }
}

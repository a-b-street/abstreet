use crate::colors::ColorScheme;
use crate::objects::{DrawCtx, RenderingHints, ID};
use crate::plugins::{AmbientPlugin, BlockingPlugin, PluginCtx};
use crate::render::DrawMap;
use abstutil::MeasureMemory;
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

    // These are all mutually exclusive and, if present, override everything else.
    pub exclusive_blocking_plugin: Option<Box<BlockingPlugin>>,

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
            exclusive_blocking_plugin: None,
            enable_debug_controls,
            cs,
        }
    }

    pub fn color_obj(&self, id: ID, ctx: &DrawCtx) -> Option<Color> {
        if let Some(ref plugin) = self.exclusive_blocking_plugin {
            return plugin.color_for(id, ctx);
        }

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
        let plugins = PluginsPerMap {
            ambient_plugins: Vec::new(),
        };
        (state, plugins)
    }
}

// Anything that holds onto any kind of ID has to live here!
pub struct PluginsPerMap {
    ambient_plugins: Vec<Box<AmbientPlugin>>,
}

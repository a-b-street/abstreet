use crate::colors::ColorScheme;
use crate::objects::ID;
use crate::render::DrawMap;
use abstutil::MeasureMemory;
use ezgui::Prerender;
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
    pub cs: ColorScheme,
}

impl UIState {
    pub fn new(flags: Flags, prerender: &Prerender) -> UIState {
        let cs = ColorScheme::load().unwrap();
        let primary = PerMapUI::new(flags, &cs, prerender);
        UIState { primary, cs }
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
    pub fn new(flags: Flags, cs: &ColorScheme, prerender: &Prerender) -> PerMapUI {
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

        PerMapUI {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags.clone(),
        }
    }
}

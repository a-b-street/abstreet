#[macro_use]
extern crate log;

use abstutil::CmdArgs;
use map_gui::options::Options;
use sim::SimFlags;
use widgetry::{EventCtx, State};

use crate::app::{App, Flags};
use crate::pregame::TitleScreen;
use crate::sandbox::{GameplayMode, SandboxMode};

mod app;
mod challenges;
mod common;
mod cutscene;
mod debug;
mod devtools;
mod edit;
mod game;
mod helpers;
mod info;
mod layer;
mod pregame;
mod sandbox;

pub fn main(mut args: CmdArgs) {
    if args.enabled("--prebake") {
        challenges::prebake_all();
        return;
    }
    let mut flags = Flags {
        sim_flags: SimFlags::from_args(&mut args),
        live_map_edits: args.enabled("--live_map_edits"),
    };
    let mut opts = Options::default();
    opts.update_from_args(&mut args);
    if args.enabled("--day_night") {
        opts.toggle_day_night_colors = true;
        opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
    }
    let mut settings = widgetry::Settings::new("A/B Street");
    settings.window_icon(abstutil::path("system/assets/pregame/icon.png"));
    if args.enabled("--dump_raw_events") {
        settings.dump_raw_events();
    }
    if let Some(s) = args.optional_parse("--scale_factor", |s| s.parse::<f64>()) {
        settings.scale_factor(s);
    }
    settings.loading_tips(map_gui::helpers::loading_tips());

    let mut mode = None;
    if let Some(x) = args.optional("--challenge") {
        let mut aliases = Vec::new();
        'OUTER: for (_, stages) in challenges::Challenge::all() {
            for challenge in stages {
                if challenge.alias == x {
                    flags.sim_flags.load = challenge.gameplay.map_name().path();
                    mode = Some(challenge.gameplay);
                    break 'OUTER;
                } else {
                    aliases.push(challenge.alias);
                }
            }
        }
        if mode.is_none() {
            panic!("Invalid --challenge={}. Choices: {}", x, aliases.join(", "));
        }
    }
    if let Some(n) = args.optional_parse("--tutorial", |s| s.parse::<usize>()) {
        mode = Some(sandbox::GameplayMode::Tutorial(
            sandbox::TutorialPointer::new(n - 1, 0),
        ));
    }

    // Don't keep the scenario modifiers in the original sim_flags; they shouldn't apply to
    // other scenarios loaed in the UI later.
    let modifiers = flags.sim_flags.modifiers.drain(..).collect();

    if mode.is_none() && flags.sim_flags.load.contains("scenarios/") {
        let (map_name, scenario) = abstutil::parse_scenario_path(&flags.sim_flags.load);
        flags.sim_flags.load = map_name.path();
        mode = Some(sandbox::GameplayMode::PlayScenario(
            map_name, scenario, modifiers,
        ));
    }
    let start_with_edits = args.optional("--edits");
    let osm_viewer = args.enabled("--osm");
    let fifteen_min = args.enabled("--15min");

    args.done();

    widgetry::run(settings, |ctx| {
        setup_app(
            ctx,
            flags,
            opts,
            start_with_edits,
            mode,
            osm_viewer,
            fifteen_min,
        )
    });
}

fn setup_app(
    ctx: &mut EventCtx,
    flags: Flags,
    opts: Options,
    start_with_edits: Option<String>,
    maybe_mode: Option<GameplayMode>,
    osm_viewer: bool,
    fifteen_min: bool,
) -> (App, Vec<Box<dyn State<App>>>) {
    let title = !opts.dev
        && !flags.sim_flags.load.contains("player/save")
        && !flags.sim_flags.load.contains("/scenarios/")
        && !osm_viewer
        && !fifteen_min
        && maybe_mode.is_none();
    let mut app = App::new(flags, opts, ctx, title);

    // Handle savestates
    let savestate = if app
        .primary
        .current_flags
        .sim_flags
        .load
        .contains("player/saves/")
    {
        assert!(maybe_mode.is_none());
        Some(app.primary.clear_sim())
    } else {
        None
    };

    // Just apply this here, don't plumb to SimFlags or anything else. We recreate things using
    // these flags later, but we don't want to keep applying the same edits.
    if let Some(edits_name) = start_with_edits {
        // TODO Maybe loading screen
        let mut timer = abstutil::Timer::new("apply initial edits");
        let edits = map_model::MapEdits::load(
            &app.primary.map,
            abstutil::path_edits(app.primary.map.get_name(), &edits_name),
            &mut timer,
        )
        .unwrap();
        crate::edit::apply_map_edits(ctx, &mut app, edits);
        app.primary
            .map
            .recalculate_pathfinding_after_edits(&mut timer);
        app.primary.clear_sim();
    }

    let states: Vec<Box<dyn State<App>>> = if title {
        vec![Box::new(TitleScreen::new(ctx, &mut app))]
    } else if osm_viewer {
        vec![crate::devtools::osm_viewer::Viewer::new(ctx, &mut app)]
    } else if fifteen_min {
        vec![crate::devtools::fifteen_min::Viewer::random_start(
            ctx, &app,
        )]
    } else {
        let mode = maybe_mode
            .unwrap_or_else(|| GameplayMode::Freeform(app.primary.map.get_name().clone()));
        vec![SandboxMode::simple_new(ctx, &mut app, mode)]
    };
    if let Some(ss) = savestate {
        // TODO This is weird, we're left in Freeform mode with the wrong UI. Can't instantiate
        // PlayScenario without clobbering.
        app.primary.sim = ss;
    }

    (app, states)
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run() {
    console_log::init_with_level(log::Level::Debug).unwrap();

    if cfg!(feature = "osm_viewer") {
        main(CmdArgs::from_args(vec!["--osm".to_string()]))
    } else {
        main(CmdArgs::new());
    }
}

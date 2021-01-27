#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use abstio::MapName;
use abstutil::{CmdArgs, Timer};
use map_gui::options::Options;
use map_model::Map;
use sim::{Sim, SimFlags};
use widgetry::{EventCtx, State, Transition};

use crate::app::{App, Flags};
use crate::pregame::TitleScreen;
use crate::sandbox::{GameplayMode, SandboxMode};

mod app;
mod challenges;
mod common;
mod debug;
mod devtools;
mod edit;
mod info;
mod layer;
mod pregame;
mod sandbox;

pub fn main() {
    let mut args = CmdArgs::new();
    if args.enabled("--prebake") {
        challenges::prebake::prebake_all();
        return;
    }
    let mut flags = Flags {
        sim_flags: SimFlags::from_args(&mut args),
        live_map_edits: args.enabled("--live_map_edits"),
        study_area: args.optional("--study_area"),
    };
    let mut opts = Options::default();
    opts.toggle_day_night_colors = true;
    opts.update_from_args(&mut args);
    let mut settings = widgetry::Settings::new("A/B Street")
        .read_svg(Box::new(abstio::slurp_bytes))
        .window_icon(abstio::path("system/assets/pregame/icon.png"))
        .loading_tips(map_gui::tools::loading_tips());
    if args.enabled("--dump_raw_events") {
        settings = settings.dump_raw_events();
    }
    if let Some(s) = args.optional_parse("--scale_factor", |s| s.parse::<f64>()) {
        settings = settings.scale_factor(s);
    }

    let mut mode = None;
    let mut initialize_tutorial = false;
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
        initialize_tutorial = true;
        mode = Some(sandbox::GameplayMode::Tutorial(
            sandbox::TutorialPointer::new(n - 1, 0),
        ));
    }

    // Don't keep the scenario modifiers in the original sim_flags; they shouldn't apply to
    // other scenarios loaed in the UI later.
    let modifiers = flags.sim_flags.modifiers.drain(..).collect();

    if mode.is_none() && flags.sim_flags.load.contains("scenarios/") {
        let (map_name, scenario) = abstio::parse_scenario_path(&flags.sim_flags.load);
        flags.sim_flags.load = map_name.path();
        mode = Some(sandbox::GameplayMode::PlayScenario(
            map_name, scenario, modifiers,
        ));
    }
    let start_with_edits = args.optional("--edits");

    args.done();

    widgetry::run(settings, |ctx| {
        setup_app(
            ctx,
            flags,
            opts,
            start_with_edits,
            mode,
            initialize_tutorial,
        )
    });
}

fn setup_app(
    ctx: &mut EventCtx,
    flags: Flags,
    mut opts: Options,
    start_with_edits: Option<String>,
    maybe_mode: Option<GameplayMode>,
    initialize_tutorial: bool,
) -> (App, Vec<Box<dyn State<App>>>) {
    let title = !opts.dev
        && !flags.sim_flags.load.contains("player/save")
        && !flags.sim_flags.load.contains("/scenarios/")
        && maybe_mode.is_none();
    // If we're starting directly in sandbox mode, usually time is midnight, so save some effort
    // and start with the correct color scheme. If we're loading a savestate and it's actually
    // daytime, we'll pay a small penalty to switch colors.
    if !title {
        opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
    }
    let cs = map_gui::colors::ColorScheme::new(ctx, opts.color_scheme);

    // SimFlags::load doesn't know how to do async IO, which we need on the web. But in the common
    // case, all we're creating there is a map. If so, use the proper async interface.
    //
    // Note if we started with a scenario, main() rewrote it to be the appropriate map, along with
    // maybe_mode.
    if flags.sim_flags.load.contains("/maps/") {
        // Get App created with a dummy blank map
        let map = Map::blank();
        let sim = Sim::new(&map, flags.sim_flags.opts.clone());
        let primary = crate::app::PerMap::map_loaded(
            map,
            sim,
            flags,
            &opts,
            &cs,
            ctx,
            &mut Timer::throwaway(),
        );
        let app = App {
            primary,
            cs,
            opts,
            per_obj: crate::app::PerObjectActions::new(),
            session: crate::app::SessionState::empty(),
        };
        let map_name = MapName::from_path(&app.primary.current_flags.sim_flags.load).unwrap();
        let states = vec![map_gui::load::MapLoader::new(
            ctx,
            &app,
            map_name,
            Box::new(move |ctx, app| {
                Transition::Clear(finish_app_setup(
                    ctx,
                    app,
                    title,
                    start_with_edits,
                    maybe_mode,
                    initialize_tutorial,
                ))
            }),
        )];
        (app, states)
    } else {
        // We're loading a savestate or a RawMap. Do it with blocking IO. This won't
        // work on the web.
        let primary = ctx.loading_screen("load map", |ctx, mut timer| {
            assert!(flags.sim_flags.modifiers.is_empty());
            let (map, sim, _) = flags.sim_flags.load(timer);
            crate::app::PerMap::map_loaded(map, sim, flags, &opts, &cs, ctx, &mut timer)
        });
        let mut app = App {
            primary,
            cs,
            opts,
            per_obj: crate::app::PerObjectActions::new(),
            session: crate::app::SessionState::empty(),
        };
        let states = finish_app_setup(
            ctx,
            &mut app,
            title,
            start_with_edits,
            maybe_mode,
            initialize_tutorial,
        );
        (app, states)
    }
}

fn finish_app_setup(
    ctx: &mut EventCtx,
    app: &mut App,
    title: bool,
    start_with_edits: Option<String>,
    maybe_mode: Option<GameplayMode>,
    initialize_tutorial: bool,
) -> Vec<Box<dyn State<App>>> {
    app.primary.init_camera_for_loaded_map(ctx, title);

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
        let mut timer = Timer::new("apply initial edits");
        let edits = map_model::MapEdits::load(
            &app.primary.map,
            abstio::path_edits(app.primary.map.get_name(), &edits_name),
            &mut timer,
        )
        .unwrap();
        crate::edit::apply_map_edits(ctx, app, edits);
        app.primary
            .map
            .recalculate_pathfinding_after_edits(&mut timer);
        app.primary.clear_sim();
    }

    if initialize_tutorial {
        crate::sandbox::gameplay::Tutorial::initialize(ctx, app);
    }

    let states: Vec<Box<dyn State<App>>> = if title {
        vec![Box::new(TitleScreen::new(ctx, app))]
    } else {
        let mode = maybe_mode
            .unwrap_or_else(|| GameplayMode::Freeform(app.primary.map.get_name().clone()));
        vec![SandboxMode::simple_new(ctx, app, mode)]
    };
    if let Some(ss) = savestate {
        // TODO This is weird, we're left in Freeform mode with the wrong UI. Can't instantiate
        // PlayScenario without clobbering.
        app.primary.sim = ss;
    }

    states
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn run() {
    main();
}

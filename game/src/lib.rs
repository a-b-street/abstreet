// Disable some noisy lints
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use abstio::MapName;
use abstutil::{CmdArgs, Timer};
use geom::Duration;
use map_gui::options::Options;
use map_gui::tools::URLManager;
use map_model::Map;
use sim::{Analytics, Sim, SimFlags};
use widgetry::{EventCtx, Settings, State, Transition};

use crate::app::{App, Flags};
use crate::common::jump_to_time_upon_startup;
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
    let settings = Settings::new("A/B Street");
    run(settings);
}

fn run(mut settings: Settings) {
    settings = settings
        .read_svg(Box::new(abstio::slurp_bytes))
        .window_icon(abstio::path("system/assets/pregame/icon.png"))
        .loading_tips(map_gui::tools::loading_tips())
        // This is approximately how much the 3 top panels in sandbox mode require.
        .require_minimum_width(1500.0);

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
    let center_camera = args.optional("--cam");
    let start_time = args.optional_parse("--time", |t| Duration::parse(t));

    if let Some(site) = args.optional("--actdev") {
        // Handle if the site was accidentally passed in with underscores. Otherwise, some study
        // areas won't be found!
        let site = site.replace("_", "-");
        let city = site.replace("-", "_");
        let name = MapName::new("gb", &city, "center");
        flags.sim_flags.load = name.path();
        flags.study_area = Some(site);
        // Parking data in the actdev maps is nonexistent, so many people have convoluted walking
        // routes just to fetch their car. Just disable parking entirely.
        flags.sim_flags.opts.infinite_parking = true;
        let scenario = if args.optional("--actdev_scenario") == Some("go_active".to_string()) {
            "go_active".to_string()
        } else {
            "base".to_string()
        };
        mode = Some(sandbox::GameplayMode::Actdev(name, scenario, false));
    }

    args.done();

    widgetry::run(settings, |ctx| {
        setup_app(
            ctx,
            flags,
            opts,
            start_with_edits,
            mode,
            initialize_tutorial,
            center_camera,
            start_time,
        )
    });
}

fn setup_app(
    ctx: &mut EventCtx,
    mut flags: Flags,
    mut opts: Options,
    start_with_edits: Option<String>,
    maybe_mode: Option<GameplayMode>,
    initialize_tutorial: bool,
    center_camera: Option<String>,
    start_time: Option<Duration>,
) -> (App, Vec<Box<dyn State<App>>>) {
    let title = !opts.dev
        && !flags.sim_flags.load.contains("player/save")
        && !flags.sim_flags.load.contains("/scenarios/")
        && maybe_mode.is_none();

    // Load the map used previously if we're starting on the title screen without any overrides.
    if title && flags.sim_flags.load == MapName::seattle("montlake").path() {
        if let Ok(default) = abstio::maybe_read_json::<map_gui::tools::DefaultMap>(
            abstio::path_player("maps.json"),
            &mut Timer::throwaway(),
        ) {
            flags.sim_flags.load = default.last_map.path();
        }
    }

    // If we're starting directly in a challenge mode, the tutorial, or by playing a scenario,
    // usually time is midnight, so save some effort and start with the correct color scheme. If
    // we're loading a savestate and it's actually daytime, we'll pay a small penalty to switch
    // colors.
    if let Some(GameplayMode::PlayScenario(_, _, _))
    | Some(GameplayMode::FixTrafficSignals)
    | Some(GameplayMode::OptimizeCommute(_, _))
    | Some(GameplayMode::Tutorial(_)) = maybe_mode
    {
        opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
    }
    if title {
        opts.color_scheme = map_gui::colors::ColorSchemeChoice::Pregame;
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
        let states = vec![map_gui::load::MapLoader::new_state(
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
                    center_camera,
                    start_time,
                ))
            }),
        )];
        (app, states)
    } else {
        // We're loading a savestate or a RawMap. Do it with blocking IO. This won't
        // work on the web.
        let primary = ctx.loading_screen("load map", |ctx, mut timer| {
            assert!(flags.sim_flags.modifiers.is_empty());
            let (map, sim, _) = flags.sim_flags.load_synchronously(timer);
            crate::app::PerMap::map_loaded(map, sim, flags, &opts, &cs, ctx, &mut timer)
        });
        let mut app = App {
            primary,
            cs,
            opts,
            per_obj: crate::app::PerObjectActions::new(),
            session: crate::app::SessionState::empty(),
        };

        let scenario_name = app.primary.sim.get_run_name().to_string();
        if let Ok(prebaked) = abstio::maybe_read_binary::<Analytics>(
            abstio::path_prebaked_results(app.primary.map.get_name(), &scenario_name),
            &mut Timer::throwaway(),
        ) {
            app.set_prebaked(Some((
                app.primary.map.get_name().clone(),
                scenario_name,
                prebaked,
            )));
        }

        let states = finish_app_setup(
            ctx,
            &mut app,
            title,
            start_with_edits,
            maybe_mode,
            initialize_tutorial,
            center_camera,
            start_time,
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
    center_camera: Option<String>,
    start_time: Option<Duration>,
) -> Vec<Box<dyn State<App>>> {
    if let Some((pt, zoom)) =
        center_camera.and_then(|cam| URLManager::parse_center_camera(app, cam))
    {
        ctx.canvas.cam_zoom = zoom;
        ctx.canvas.center_on_map_pt(pt);
    } else {
        app.primary.init_camera_for_loaded_map(ctx, title);
    }

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
    } else if let Some(mode) = maybe_mode {
        if let GameplayMode::Actdev(_, _, _) = mode {
            vec![SandboxMode::async_new(
                app,
                mode,
                jump_to_time_upon_startup(Duration::hours(8)),
            )]
        } else if let Some(t) = start_time {
            vec![SandboxMode::async_new(
                app,
                mode,
                jump_to_time_upon_startup(t),
            )]
        } else {
            vec![SandboxMode::simple_new(app, mode)]
        }
    } else {
        // We got here by just passing --dev and a map as flags; we're just looking at an empty
        // map. Start in the daytime.
        vec![SandboxMode::async_new(
            app,
            GameplayMode::Freeform(app.primary.map.get_name().clone()),
            jump_to_time_upon_startup(Duration::hours(6)),
        )]
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
#[wasm_bindgen(js_name = "run")]
pub fn run_wasm(root_dom_id: String, assets_base_url: String, assets_are_gzipped: bool) {
    let settings = Settings::new("A/B Street")
        .root_dom_element_id(root_dom_id)
        .assets_base_url(assets_base_url)
        .assets_are_gzipped(assets_are_gzipped);

    run(settings);
}

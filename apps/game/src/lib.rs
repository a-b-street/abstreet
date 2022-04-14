// Disable some noisy lints
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate log;

use structopt::StructOpt;

use abstio::MapName;
use abstutil::Timer;
use geom::Duration;
use map_gui::colors::ColorSchemeChoice;
use map_gui::options::Options;
use map_model::{Map, MapEdits};
use sim::Sim;
use synthpop::Scenario;
use widgetry::tools::{FutureLoader, PopupMsg, URLManager};
use widgetry::{EventCtx, Settings, State, Transition};

use crate::app::{App, Flags, PerMap};
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
mod ungap;

pub fn main() {
    let settings = Settings::new("A/B Street");
    run(settings);
}

#[derive(StructOpt)]
#[structopt(name = "abstreet", about = "The A/B Street traffic simulator")]
struct Args {
    #[structopt(flatten)]
    flags: Flags,
    /// Start with these map edits loaded. This should be the name of edits, not a full path.
    #[structopt(long = "edits")]
    start_with_edits: Option<String>,
    /// Initially position the camera here. The format is an OSM-style `zoom/lat/lon` string
    /// (https://wiki.openstreetmap.org/wiki/Browsing#Other_URL_tricks).
    #[structopt(long)]
    cam: Option<String>,
    /// Start the simulation at this time
    #[structopt(long = "time", parse(try_from_str = Duration::parse))]
    start_time: Option<Duration>,
    /// Load the map at this path as a secondary debug map to compare to the main one
    #[structopt(long = "diff")]
    diff_map: Option<String>,
    /// Print raw widgetry events to the console for debugging
    #[structopt(long)]
    dump_raw_events: bool,
    /// Override the monitor's auto-detected scale factor
    #[structopt(long)]
    scale_factor: Option<f64>,

    /// Dev mode exposes experimental tools useful for debugging, but that'd likely confuse most
    /// players.
    #[structopt(long)]
    dev: bool,
    /// The color scheme for map elements, agents, and the UI.
    #[structopt(long, parse(try_from_str = ColorSchemeChoice::parse))]
    color_scheme: Option<ColorSchemeChoice>,
    /// When making a screen recording, enable this option to hide some UI elements
    #[structopt(long)]
    minimal_controls: bool,

    /// Run a configured set of simulations and record prebaked data.
    #[structopt(long)]
    prebake: bool,

    /// Start at the tutorial intro screen
    #[structopt(long)]
    tutorial_intro: bool,
    /// Start by listing gameplay challenges
    #[structopt(long)]
    challenges: bool,
    /// Start in the simulation sandbox mode
    #[structopt(long)]
    sandbox: bool,
    /// Start by showing community proposals
    #[structopt(long)]
    proposals: bool,
    /// Launch Ungap the Map, a bike network planning tool
    #[structopt(long)]
    ungap: bool,
    /// Start by listing internal developer tools
    #[structopt(long)]
    devtools: bool,
    /// Start by showing this KMl file in a debug viewer
    #[structopt(long = "kml")]
    load_kml: Option<String>,
    /// Start playing a particular challenge
    #[structopt(long)]
    challenge: Option<String>,
    /// Start on a particular tutorial stage
    #[structopt(long)]
    tutorial: Option<usize>,
    /// Start in ActDev mode for a particular site name.
    #[structopt(long)]
    actdev: Option<String>,
    /// Start by showing an ActDev scenario. Either "base" or "go_active".
    #[structopt(long)]
    actdev_scenario: Option<String>,
    /// Start in a tool for comparing traffic counts
    #[structopt(long)]
    compare_counts: Option<Vec<String>>,
}

struct Setup {
    flags: Flags,
    opts: Options,
    start_with_edits: Option<String>,
    initialize_tutorial: bool,
    center_camera: Option<String>,
    start_time: Option<Duration>,
    diff_map: Option<String>,
    mode: Mode,
}

// TODO Switch to explicit enum subcommands, each of which includes precisely the set of common
// flags that're valid for that mode
#[derive(PartialEq)]
enum Mode {
    SomethingElse,
    TutorialIntro,
    Challenges,
    Sandbox,
    Proposals,
    Ungap,
    Devtools,
    LoadKML(String),
    CompareCounts(String, String),
    Gameplay(GameplayMode),
}

fn run(mut settings: Settings) {
    abstutil::logger::setup();

    settings = settings
        .read_svg(Box::new(abstio::slurp_bytes))
        .window_icon(abstio::path("system/assets/pregame/icon.png"))
        .loading_tips(map_gui::tools::loading_tips())
        // This is approximately how much the 3 top panels in sandbox mode require.
        .require_minimum_width(1500.0);

    let mut args = Args::from_iter(abstutil::cli_args());
    args.flags.sim_flags.initialize();

    if args.prebake {
        challenges::prebake::prebake_all();
        return;
    }

    let mut setup = Setup {
        flags: args.flags,
        opts: Options::load_or_default(),
        start_with_edits: args.start_with_edits,
        initialize_tutorial: false,
        center_camera: args.cam,
        start_time: args.start_time,
        diff_map: args.diff_map,
        mode: if args.tutorial_intro {
            Mode::TutorialIntro
        } else if args.challenges {
            Mode::Challenges
        } else if args.sandbox {
            Mode::Sandbox
        } else if args.proposals {
            Mode::Proposals
        } else if args.ungap {
            Mode::Ungap
        } else if args.devtools {
            Mode::Devtools
        } else if let Some(kml) = args.load_kml {
            Mode::LoadKML(kml)
        } else if let Some(mut paths) = args.compare_counts {
            if paths.len() != 2 {
                panic!("--compare-counts takes exactly two paths");
            }
            Mode::CompareCounts(paths.remove(0), paths.remove(0))
        } else {
            Mode::SomethingElse
        },
    };

    setup.opts.toggle_day_night_colors = true;
    // Update options from CLI flags
    setup.opts.dev = args.dev;
    setup.opts.minimal_controls = args.minimal_controls;
    if let Some(cs) = args.color_scheme {
        setup.opts.color_scheme = cs;
        setup.opts.toggle_day_night_colors = false;
    }

    settings = settings.canvas_settings(setup.opts.canvas_settings.clone());

    if args.dump_raw_events {
        settings = settings.dump_raw_events();
    }
    if let Some(s) = args.scale_factor {
        settings = settings.scale_factor(s);
    }

    if let Some(x) = args.challenge {
        // TODO This is a weak form of mutual exclusion; just use subcommands
        assert!(setup.mode == Mode::SomethingElse);
        let mut aliases = Vec::new();
        'OUTER: for (_, stages) in challenges::Challenge::all() {
            for challenge in stages {
                if challenge.alias == x {
                    setup.flags.sim_flags.load = challenge.gameplay.map_name().path();
                    setup.mode = Mode::Gameplay(challenge.gameplay);
                    break 'OUTER;
                } else {
                    aliases.push(challenge.alias);
                }
            }
        }
        if setup.mode == Mode::SomethingElse {
            panic!("Invalid --challenge={}. Choices: {}", x, aliases.join(", "));
        }
    }
    if let Some(n) = args.tutorial {
        setup.initialize_tutorial = true;
        setup.mode = Mode::Gameplay(sandbox::GameplayMode::Tutorial(
            sandbox::TutorialPointer::new(n - 1, 0),
        ));
    }

    // Don't keep the scenario modifiers in the original sim_flags; they shouldn't apply to
    // other scenarios loaed in the UI later.
    let modifiers = setup.flags.sim_flags.scenario_modifiers.drain(..).collect();

    if setup.mode == Mode::SomethingElse && setup.flags.sim_flags.load.contains("scenarios/") {
        let (map_name, scenario) = abstio::parse_scenario_path(&setup.flags.sim_flags.load);
        setup.flags.sim_flags.load = map_name.path();
        setup.mode = Mode::Gameplay(sandbox::GameplayMode::PlayScenario(
            map_name, scenario, modifiers,
        ));
    }

    if let Some(site) = args.actdev {
        // Handle if the site was accidentally passed in with underscores. Otherwise, some study
        // areas won't be found!
        let site = site.replace("_", "-");
        let city = site.replace("-", "_");
        let name = MapName::new("gb", &city, "center");
        setup.flags.sim_flags.load = name.path();
        setup.flags.study_area = Some(site);
        // Parking data in the actdev maps is nonexistent, so many people have convoluted walking
        // routes just to fetch their car. Just disable parking entirely.
        setup.flags.sim_flags.opts.infinite_parking = true;
        let scenario = if args.actdev_scenario == Some("go_active".to_string()) {
            "go_active".to_string()
        } else {
            "base".to_string()
        };
        setup.mode = Mode::Gameplay(sandbox::GameplayMode::Actdev(name, scenario, false));
    }

    widgetry::run(settings, |ctx| setup_app(ctx, setup))
}

fn setup_app(ctx: &mut EventCtx, mut setup: Setup) -> (App, Vec<Box<dyn State<App>>>) {
    let title = !setup.opts.dev
        && !setup.flags.sim_flags.load.contains("player/save")
        && !setup.flags.sim_flags.load.contains("/scenarios/")
        && setup.mode == Mode::SomethingElse;

    // Load the map used previously if we're starting on the title screen without any overrides.
    if title && setup.flags.sim_flags.load == MapName::seattle("montlake").path() {
        if let Ok(default) = abstio::maybe_read_json::<map_gui::tools::DefaultMap>(
            abstio::path_player("maps.json"),
            &mut Timer::throwaway(),
        ) {
            setup.flags.sim_flags.load = default.last_map.path();
        }
    }

    // If we're starting directly in a challenge mode, the tutorial, or by playing a scenario,
    // usually time is midnight, so save some effort and start with the correct color scheme. If
    // we're loading a savestate and it's actually daytime, we'll pay a small penalty to switch
    // colors.
    if let Mode::Gameplay(
        GameplayMode::PlayScenario(_, _, _)
        | GameplayMode::FixTrafficSignals
        | GameplayMode::OptimizeCommute(_, _)
        | GameplayMode::Tutorial(_),
    ) = setup.mode
    {
        setup.opts.color_scheme = map_gui::colors::ColorSchemeChoice::NightMode;
    }
    if setup.mode != Mode::SomethingElse {
        setup.opts.color_scheme = map_gui::colors::ColorSchemeChoice::DayMode;
    }
    let cs = map_gui::colors::ColorScheme::new(ctx, setup.opts.color_scheme);

    // No web support; this uses blocking IO
    let secondary = setup.diff_map.as_ref().map(|path| {
        ctx.loading_screen("load secondary map", |ctx, timer| {
            // Use this low-level API, since the secondary map file probably isn't in the usual
            // directory structure
            let mut map: Map = abstio::read_binary(path.clone(), timer);
            map.map_loaded_directly(timer);
            let sim = Sim::new(&map, setup.flags.sim_flags.opts.clone());
            let mut per_map =
                PerMap::map_loaded(map, sim, setup.flags.clone(), &setup.opts, &cs, ctx, timer);
            per_map.is_secondary = true;
            per_map
        })
    });

    // SimFlags::load doesn't know how to do async IO, which we need on the web. But in the common
    // case, all we're creating there is a map. If so, use the proper async interface.
    //
    // Note if we started with a scenario, main() rewrote it to be the appropriate map, along with
    // mode.
    if setup.flags.sim_flags.load.contains("/maps/") {
        // Get App created with a dummy blank map
        let map = Map::blank();
        let sim = Sim::new(&map, setup.flags.sim_flags.opts.clone());
        let primary = PerMap::map_loaded(
            map,
            sim,
            setup.flags.clone(),
            &setup.opts,
            &cs,
            ctx,
            &mut Timer::throwaway(),
        );
        let app = App {
            primary,
            secondary: None,
            store_unedited_map_in_secondary: false,
            cs,
            opts: setup.opts.clone(),
            per_obj: crate::app::PerObjectActions::new(),
            session: crate::app::SessionState::empty(),
        };
        let map_name = MapName::from_path(&app.primary.current_flags.sim_flags.load).unwrap();
        let states = vec![map_gui::load::MapLoader::new_state(
            ctx,
            &app,
            map_name,
            Box::new(move |ctx, app| {
                Transition::Clear(continue_app_setup(ctx, app, title, setup, secondary))
            }),
        )];
        (app, states)
    } else {
        // We're loading a savestate or a RawMap. Do it with blocking IO. This won't
        // work on the web.
        let primary = ctx.loading_screen("load map", |ctx, timer| {
            assert!(setup.flags.sim_flags.scenario_modifiers.is_empty());
            let (map, sim, _) = setup.flags.sim_flags.load_synchronously(timer);
            PerMap::map_loaded(map, sim, setup.flags.clone(), &setup.opts, &cs, ctx, timer)
        });
        assert!(secondary.is_none());
        let mut app = App {
            primary,
            secondary: None,
            store_unedited_map_in_secondary: false,
            cs,
            opts: setup.opts.clone(),
            per_obj: crate::app::PerObjectActions::new(),
            session: crate::app::SessionState::empty(),
        };

        let states = continue_app_setup(ctx, &mut app, title, setup, None);
        (app, states)
    }
}

fn continue_app_setup(
    ctx: &mut EventCtx,
    app: &mut App,
    title: bool,
    setup: Setup,
    secondary: Option<PerMap>,
) -> Vec<Box<dyn State<App>>> {
    // Run this after loading the primary map. That process wipes out app.secondary.
    app.secondary = secondary;

    if !URLManager::change_camera(
        ctx,
        setup.center_camera.as_ref(),
        app.primary.map.get_gps_bounds(),
    ) {
        app.primary.init_camera_for_loaded_map(ctx);
    }

    // Handle savestates
    let savestate = if app
        .primary
        .current_flags
        .sim_flags
        .load
        .contains("player/saves/")
    {
        assert!(setup.mode == Mode::SomethingElse);
        Some(app.primary.clear_sim())
    } else {
        None
    };

    // Just apply this here, don't plumb to SimFlags or anything else. We recreate things using
    // these flags later, but we don't want to keep applying the same edits.
    if let Some(ref edits_name) = setup.start_with_edits {
        // Remote edits require another intermediate state to load
        if let Some(id) = edits_name.strip_prefix("remote/") {
            let (_, outer_progress_rx) = futures_channel::mpsc::channel(1);
            let (_, inner_progress_rx) = futures_channel::mpsc::channel(1);
            let url = format!("{}/get?id={}", crate::common::share::PROPOSAL_HOST_URL, id);
            return vec![FutureLoader::<App, Vec<u8>>::new_state(
                ctx,
                Box::pin(async move {
                    let bytes = abstio::http_get(url).await?;
                    let wrapper: Box<dyn Send + FnOnce(&App) -> Vec<u8>> = Box::new(move |_| bytes);
                    Ok(wrapper)
                }),
                outer_progress_rx,
                inner_progress_rx,
                "Downloading proposal",
                Box::new(move |ctx, app, result| {
                    match result
                        .and_then(|bytes| MapEdits::load_from_bytes(&app.primary.map, bytes))
                    {
                        Ok(edits) => Transition::Clear(finish_app_setup(
                            ctx,
                            app,
                            title,
                            savestate,
                            Some(edits),
                            setup,
                        )),
                        Err(err) => {
                            // TODO Fail more gracefully -- add a popup with the error, but continue
                            // app setup?
                            error!("Couldn't load remote proposal: {}", err);
                            Transition::Replace(PopupMsg::new_state(
                                ctx,
                                "Couldn't load remote proposal",
                                vec![err.to_string()],
                            ))
                        }
                    }
                }),
            )];
        }

        for path in [
            abstio::path_edits(app.primary.map.get_name(), edits_name),
            abstio::path(format!("system/proposals/{}.json", edits_name)),
        ] {
            if abstio::file_exists(&path) {
                let edits = map_model::MapEdits::load_from_file(
                    &app.primary.map,
                    path,
                    &mut Timer::throwaway(),
                )
                .unwrap();
                return finish_app_setup(ctx, app, title, savestate, Some(edits), setup);
            }
        }

        // TODO Fail more gracefully -- add a popup with the error, but continue app setup?
        panic!("Can't start with nonexistent edits {}", edits_name);
    }

    finish_app_setup(ctx, app, title, savestate, None, setup)
}

fn finish_app_setup(
    ctx: &mut EventCtx,
    app: &mut App,
    title: bool,
    savestate: Option<Sim>,
    edits: Option<MapEdits>,
    setup: Setup,
) -> Vec<Box<dyn State<App>>> {
    if setup.mode == Mode::Ungap {
        app.store_unedited_map_in_secondary = true;
    }
    if let Some(edits) = edits {
        ctx.loading_screen("apply initial edits", |ctx, timer| {
            crate::edit::apply_map_edits(ctx, app, edits);
            app.primary.map.recalculate_pathfinding_after_edits(timer);
            app.primary.clear_sim();
        });
    }

    if setup.initialize_tutorial {
        crate::sandbox::gameplay::Tutorial::initialize(ctx, app);
    }

    if title {
        return vec![TitleScreen::new_state(ctx, app)];
    }

    let state = if let Some(ss) = savestate {
        app.primary.sim = ss;
        SandboxMode::start_from_savestate(app)
    } else {
        match setup.mode {
            Mode::Gameplay(gameplay) => {
                if let GameplayMode::Actdev(_, _, _) = gameplay {
                    SandboxMode::async_new(
                        app,
                        gameplay,
                        jump_to_time_upon_startup(Duration::hours(8)),
                    )
                } else if let Some(t) = setup.start_time {
                    SandboxMode::async_new(app, gameplay, jump_to_time_upon_startup(t))
                } else {
                    SandboxMode::simple_new(app, gameplay)
                }
            }
            Mode::SomethingElse => {
                let start_time = setup.start_time.unwrap_or(Duration::hours(6));

                // Not attempting to keep the primary and secondary simulations synchronized at the
                // same time yet. Just handle this one startup case, so we can switch maps without
                // constantly flopping day/night mode.
                if let Some(ref mut secondary) = app.secondary {
                    secondary.sim.timed_step(
                        &secondary.map,
                        start_time,
                        &mut None,
                        &mut Timer::throwaway(),
                    );
                }

                // We got here by just passing --dev and a map as flags; we're just looking at an
                // empty map. Start in the daytime.
                SandboxMode::async_new(
                    app,
                    GameplayMode::Freeform(app.primary.map.get_name().clone()),
                    jump_to_time_upon_startup(start_time),
                )
            }
            Mode::TutorialIntro => sandbox::gameplay::Tutorial::start(ctx, app),
            Mode::Challenges => challenges::ChallengesPicker::new_state(ctx, app),
            Mode::Sandbox => SandboxMode::simple_new(
                app,
                GameplayMode::PlayScenario(
                    app.primary.map.get_name().clone(),
                    Scenario::default_scenario_for_map(app.primary.map.get_name()),
                    Vec::new(),
                ),
            ),
            Mode::Proposals => pregame::proposals::Proposals::new_state(ctx, None),
            Mode::Ungap => {
                let layers = ungap::Layers::new(ctx, app);
                ungap::ExploreMap::new_state(ctx, app, layers)
            }
            Mode::Devtools => devtools::DevToolsMode::new_state(ctx, app),
            Mode::LoadKML(path) => crate::devtools::kml::ViewKML::new_state(ctx, app, Some(path)),
            Mode::CompareCounts(path1, path2) => {
                crate::devtools::compare_counts::GenericCompareCounts::new_state(
                    ctx, app, path1, path2,
                )
            }
        }
    };
    vec![TitleScreen::new_state(ctx, app), state]
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

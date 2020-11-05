#[macro_use]
extern crate log;

use abstutil::{CmdArgs, MapName, Timer};
use geom::Duration;
use sim::SimFlags;
use widgetry::{EventCtx, State};

use crate::app::{App, Flags};
use crate::options::Options;
use crate::pregame::TitleScreen;
use crate::sandbox::{GameplayMode, SandboxMode};

mod app;
mod challenges;
mod colors;
mod common;
mod cutscene;
mod debug;
mod devtools;
mod edit;
mod game;
mod helpers;
mod info;
mod layer;
mod load;
mod options;
mod pregame;
mod render;
mod sandbox;

pub fn main(mut args: CmdArgs) {
    if args.enabled("--prebake") {
        challenges::prebake_all();
        return;
    }
    if args.enabled("--smoketest") {
        smoke_test();
        return;
    }
    if args.enabled("--check_proposals") {
        check_proposals();
        return;
    }

    let mut flags = Flags {
        sim_flags: SimFlags::from_args(&mut args),
        num_agents: args.optional_parse("--num_agents", |s| s.parse()),
        live_map_edits: args.enabled("--live_map_edits"),
    };
    let mut opts = options::Options::default();
    opts.dev = args.enabled("--dev");
    if args.enabled("--lowzoom") {
        opts.min_zoom_for_detail = 1.0;
    }

    if let Some(x) = args.optional("--color_scheme") {
        let mut ok = false;
        let mut options = Vec::new();
        for c in colors::ColorSchemeChoice::choices() {
            options.push(c.label.clone());
            if c.label == x {
                opts.color_scheme = c.data;
                ok = true;
                break;
            }
        }
        if !ok {
            panic!(
                "Invalid --color_scheme={}. Choices: {}",
                x,
                options.join(", ")
            );
        }
    }
    let mut settings = widgetry::Settings::new("A/B Street");
    settings.window_icon(abstutil::path("system/assets/pregame/icon.png"));
    if args.enabled("--dump_raw_events") {
        settings.dump_raw_events();
    }
    if let Some(s) = args.optional_parse("--scale_factor", |s| s.parse::<f64>()) {
        settings.scale_factor(s);
    }
    settings.loading_tips(helpers::loading_tips());

    let mut mode = None;
    if let Some(x) = args.optional("--challenge") {
        let mut aliases = Vec::new();
        'OUTER: for (_, stages) in challenges::Challenge::all() {
            for challenge in stages {
                if challenge.alias == x {
                    flags.sim_flags.load = abstutil::path_map(&challenge.gameplay.map_name());
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
        flags.sim_flags.load = abstutil::path_map(&map_name);
        mode = Some(sandbox::GameplayMode::PlayScenario(
            map_name, scenario, modifiers,
        ));
    }
    let start_with_edits = args.optional("--edits");
    let osm_viewer = args.enabled("--osm");

    args.done();

    widgetry::run(settings, |ctx| {
        setup_app(ctx, flags, opts, start_with_edits, mode, osm_viewer)
    });
}

fn setup_app(
    ctx: &mut EventCtx,
    flags: Flags,
    opts: Options,
    start_with_edits: Option<String>,
    maybe_mode: Option<GameplayMode>,
    osm_viewer: bool,
) -> (App, Vec<Box<dyn State<App>>>) {
    let title = !opts.dev
        && !flags.sim_flags.load.contains("player/save")
        && !flags.sim_flags.load.contains("system/scenarios")
        && !osm_viewer
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

fn smoke_test() {
    let mut timer = Timer::new("run a smoke-test for all maps");
    for name in abstutil::list_all_objects(abstutil::path_all_maps()) {
        // TODO Wrong! When we start using city as part of the filename, this'll break. But that's
        // also when path_all_maps() has to change.
        let name = MapName::seattle(&name);
        let map = map_model::Map::new(abstutil::path_map(&name), &mut timer);
        let scenario = if map.get_city_name() == "seattle" {
            abstutil::read_binary(abstutil::path_scenario(&name, "weekday"), &mut timer)
        } else {
            let mut rng = sim::SimFlags::for_test("smoke_test").make_rng();
            sim::ScenarioGenerator::proletariat_robot(&map, &mut rng, &mut timer)
        };

        let mut opts = sim::SimOptions::new("smoke_test");
        opts.alerts = sim::AlertHandler::Silence;
        let mut sim = sim::Sim::new(&map, opts, &mut timer);
        // Bit of an abuse of this, but just need to fix the rng seed.
        let mut rng = sim::SimFlags::for_test("smoke_test").make_rng();
        scenario.instantiate(&mut sim, &map, &mut rng, &mut timer);
        sim.timed_step(&map, Duration::hours(1), &mut None, &mut timer);

        if vec![
            "downtown",
            "krakow_center",
            "lakeslice",
            "montlake",
            "udistrict",
        ]
        .contains(&name.map.as_str())
        {
            dump_route_goldenfile(&map).unwrap();
        }
    }
}

fn dump_route_goldenfile(map: &map_model::Map) -> Result<(), std::io::Error> {
    use std::fs::File;
    use std::io::Write;

    let path = abstutil::path(format!(
        "route_goldenfiles/{}.txt",
        map.get_name().as_filename()
    ));
    let mut f = File::create(path)?;
    for br in map.all_bus_routes() {
        writeln!(
            f,
            "{} from {} to {:?}",
            br.osm_rel_id, br.start, br.end_border
        )?;
        for bs in &br.stops {
            let bs = map.get_bs(*bs);
            writeln!(
                f,
                "  {}: {} driving, {} sidewalk",
                bs.name, bs.driving_pos, bs.sidewalk_pos
            )?;
        }
    }
    Ok(())
}

fn check_proposals() {
    let mut timer = Timer::new("check all proposals");
    for name in abstutil::list_all_objects(abstutil::path("system/proposals")) {
        match abstutil::maybe_read_json::<map_model::PermanentMapEdits>(
            abstutil::path(format!("system/proposals/{}.json", name)),
            &mut timer,
        ) {
            Ok(perma) => {
                let map = map_model::Map::new(abstutil::path_map(&perma.map_name), &mut timer);
                if let Err(err) = map_model::PermanentMapEdits::from_permanent(perma, &map) {
                    timer.error(format!("{} is out-of-date: {}", name, err));
                }
            }
            Err(err) => {
                timer.error(format!("{} JSON is broken: {}", name, err));
            }
        }
    }
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

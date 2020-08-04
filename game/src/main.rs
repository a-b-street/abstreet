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
mod options;
mod pregame;
mod render;
mod sandbox;

use crate::app::Flags;
use abstutil::{CmdArgs, Timer};
use geom::Duration;
use sim::SimFlags;

fn main() {
    let mut args = CmdArgs::new();

    if args.enabled("--prebake") {
        challenges::prebake_all();
        return;
    }
    if args.enabled("--smoketest") {
        smoke_test();
        return;
    }

    let mut flags = Flags {
        sim_flags: SimFlags::from_args(&mut args),
        num_agents: args.optional_parse("--num_agents", |s| s.parse()),
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
    let mut settings = ezgui::Settings::new("A/B Street");
    settings.window_icon(abstutil::path("system/assets/pregame/icon.png"));
    if args.enabled("--enable_profiler") {
        settings.enable_profiling();
    }
    if args.enabled("--dump_raw_events") {
        settings.dump_raw_events();
    }
    if let Some(s) = args.optional_parse("--scale_factor", |s| s.parse::<f64>()) {
        settings.scale_factor(s);
    }

    let mut mode = None;
    if let Some(x) = args.optional("--challenge") {
        let mut aliases = Vec::new();
        'OUTER: for (_, stages) in challenges::Challenge::all() {
            for challenge in stages {
                if challenge.alias == x {
                    flags.sim_flags.load = challenge.gameplay.map_path();
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
    if mode.is_none() && flags.sim_flags.load.contains("scenarios/") {
        // TODO regex
        let parts = flags.sim_flags.load.split("/").collect::<Vec<_>>();
        let map_path = abstutil::path_map(parts[parts.len() - 2]);
        let scenario = abstutil::basename(parts[parts.len() - 1]);
        flags.sim_flags.load = map_path.clone();
        mode = Some(sandbox::GameplayMode::PlayScenario(
            map_path,
            scenario,
            Vec::new(),
        ));
    }
    let start_with_edits = args.optional("--edits");

    args.done();

    ezgui::run(settings, |ctx| {
        game::Game::new(flags, opts, start_with_edits, mode, ctx)
    });
}

fn smoke_test() {
    let mut timer = Timer::new("run a smoke-test for all maps");
    for name in abstutil::list_all_objects(abstutil::path_all_maps()) {
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
    }
}

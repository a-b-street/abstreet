mod abtest;
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
mod managed;
mod options;
mod pregame;
mod render;
mod sandbox;

use crate::app::Flags;
use abstutil::CmdArgs;
use sim::SimFlags;

fn main() {
    let mut args = CmdArgs::new();

    if args.enabled("--prebake") {
        challenges::prebake_all();
        return;
    }

    let mut flags = Flags {
        sim_flags: SimFlags::from_args(&mut args),
        kml: args.optional("--kml"),
        draw_lane_markings: !args.enabled("--dont_draw_lane_markings"),
        num_agents: args.optional_parse("--num_agents", |s| s.parse()),
    };
    let mut opts = options::Options::default();
    if args.enabled("--dev") {
        opts.dev = true;
        flags.sim_flags.rng_seed = Some(42);
    }

    // No random in wasm
    #[cfg(target_arch = "wasm32")]
    {
        flags.sim_flags.rng_seed = Some(42);
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
    let mut settings = ezgui::Settings::new("A/B Street", "../data/system/fonts");
    if args.enabled("--enable_profiler") {
        settings.enable_profiling();
    }
    if args.enabled("--dump_raw_events") {
        settings.dump_raw_events();
    }
    if let Some(n) = args.optional_parse("--font_size", |s| s.parse::<usize>()) {
        settings.default_font_size(n);
    }
    if let Some(s) = args.optional_parse("--scale_factor", |s| s.parse::<f64>()) {
        settings.scale_factor(s);
    }

    let mut mode = None;
    if let Some(x) = args.optional("--challenge") {
        let mut aliases = Vec::new();
        'OUTER: for (_, stages) in challenges::Challenge::all(true) {
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
    // TODO Stage only, not part
    if let Some(n) = args.optional_parse("--tutorial", |s| s.parse::<usize>()) {
        mode = Some(sandbox::GameplayMode::Tutorial(
            sandbox::TutorialPointer::new(n - 1, 0),
        ));
    }
    if mode.is_none() && flags.sim_flags.load.contains("scenarios/") {
        // TODO regex
        let parts = flags.sim_flags.load.split("/").collect::<Vec<_>>();
        let map_path = abstutil::path_map(parts[4]);
        let scenario = abstutil::basename(parts[5]);
        flags.sim_flags.load = map_path.clone();
        mode = Some(sandbox::GameplayMode::PlayScenario(map_path, scenario));
    }
    let start_with_edits = args.optional("--edits");

    args.done();

    ezgui::run(settings, |ctx| {
        game::Game::new(flags, opts, start_with_edits, mode, ctx)
    });
}

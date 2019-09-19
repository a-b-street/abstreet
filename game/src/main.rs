mod abtest;
mod common;
mod debug;
mod edit;
mod game;
mod helpers;
mod mission;
mod render;
mod sandbox;
mod splash_screen;
mod tutorial;
mod ui;

use crate::ui::Flags;
use abstutil::CmdArgs;
use sim::SimFlags;

fn main() {
    let mut args = CmdArgs::new();
    let mut flags = Flags {
        sim_flags: SimFlags::from_args(&mut args),
        kml: args.optional("--kml"),
        draw_lane_markings: !args.enabled("--dont_draw_lane_markings"),
        enable_profiler: args.enabled("--enable_profiler"),
        num_agents: args
            .optional("--num_agents")
            .map(|s| s.parse::<usize>().unwrap()),
        splash: !args.enabled("--no_splash"),
        textures: !args.enabled("--no_textures"),
    };
    if args.enabled("--dev") {
        flags.splash = false;
        flags.textures = false;
        flags.sim_flags.rng_seed = Some(42);
    }
    args.done();

    ezgui::run("A/B Street", 1800.0, 800.0, |ctx| {
        game::Game::new(flags, ctx)
    });
}

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil::{LogAdapter, Timer};
use log::LevelFilter;
use sim::SimFlags;
use structopt::StructOpt;

static LOG_ADAPTER: LogAdapter = LogAdapter;

#[derive(StructOpt, Debug)]
#[structopt(name = "headless")]
struct Flags {
    #[structopt(flatten)]
    sim_flags: SimFlags,

    /// Optional time to savestate
    #[structopt(long = "save_at")]
    save_at: Option<String>,
}

fn main() {
    let flags = Flags::from_args();

    log::set_max_level(LevelFilter::Debug);
    log::set_logger(&LOG_ADAPTER).unwrap();

    // TODO not the ideal way to distinguish what thing we loaded
    let load = flags.sim_flags.load.clone();
    let mut timer = Timer::new("setup headless");
    let (map, mut sim) = sim::load(
        flags.sim_flags,
        Some(sim::Tick::from_seconds(30)),
        &mut timer,
    );
    timer.done();

    if load.contains("data/raw_maps/") {
        sim.small_spawn(&map);
    }

    let save_at = if let Some(ref time_str) = flags.save_at {
        if let Some(t) = sim::Tick::parse(time_str) {
            Some(t)
        } else {
            panic!("Couldn't parse time {}", time_str);
        }
    } else {
        None
    };

    cpuprofiler::PROFILER
        .lock()
        .unwrap()
        .start("./profile")
        .unwrap();
    sim.run_until_done(
        &map,
        move |sim| {
            if Some(sim.time) == save_at {
                sim.save();
                // Some simulatiosn run for a really long time, just do this.
                cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
            }
        },
        None,
    );
    sim::save_backtraces("call_graph.json");
    println!("{:?}", sim.get_score());
}

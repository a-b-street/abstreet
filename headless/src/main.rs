// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate control;
extern crate log;
extern crate map_model;
extern crate sim;
#[macro_use]
extern crate structopt;
extern crate yansi;

use log::{LevelFilter, Log, Metadata, Record};
use structopt::StructOpt;

static LOG_ADAPTER: LogAdapter = LogAdapter;

#[derive(StructOpt, Debug)]
#[structopt(name = "headless")]
struct Flags {
    /// Map or savestate to load
    #[structopt(name = "load")]
    load: String,

    /// Optional RNG seed
    #[structopt(long = "rng_seed")]
    rng_seed: Option<u8>,

    /// Optional time to savestate
    #[structopt(long = "save_at")]
    save_at: Option<String>,

    /// Big or large random scenario?
    #[structopt(long = "big_sim")]
    big_sim: bool,

    /// Scenario name for savestating
    #[structopt(long = "scenario_name", default_value = "headless")]
    scenario_name: String,

    /// Name of map edits
    #[structopt(long = "edits_name", default_value = "no_edits")]
    edits_name: String,
}

fn main() {
    let flags = Flags::from_args();

    log::set_max_level(LevelFilter::Debug);
    log::set_logger(&LOG_ADAPTER).unwrap();

    let (map, control_map, mut sim) = sim::load(
        flags.load.clone(),
        flags.scenario_name,
        flags.edits_name,
        flags.rng_seed,
        Some(sim::Tick::from_seconds(30)),
    );

    // TODO not the ideal way to distinguish what thing we loaded
    if flags.load.contains("data/maps/") {
        if flags.big_sim {
            sim.big_spawn(&map);
        } else {
            sim.small_spawn(&map);
        }
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

    sim.run_until_done(
        &map,
        &control_map,
        Box::new(move |sim| {
            if Some(sim.time) == save_at {
                sim.save();
            }
        }),
    );
    sim::save_backtraces("call_graph.json");
    println!("{:?}", sim.get_score());
}

// TODO This is copied from editor; dedupe how?
struct LogAdapter;

impl Log for LogAdapter {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        use yansi::Paint;

        let line = format!(
            "[{}] [{}] {}",
            Paint::white(record.level()),
            match record.target() {
                "UI" => Paint::red("UI"),
                "sim" => Paint::green("sim"),
                "map" => Paint::blue("map"),
                x => Paint::cyan(x),
            },
            record.args()
        );
        println!("{}", line);
    }

    fn flush(&self) {}
}

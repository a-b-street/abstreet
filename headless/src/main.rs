// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;
#[macro_use]
extern crate structopt;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "headless")]
struct Flags {
    /// ABST input to load
    #[structopt(name = "abst_input")]
    abst_input: String,

    /// Optional RNG seed
    #[structopt(long = "rng_seed")]
    rng_seed: Option<u8>,

    /// Optional time to savestate
    #[structopt(long = "save_at")]
    save_at: Option<u32>,

    /// Optional savestate to load
    #[structopt(long = "load_from")]
    load_from: Option<String>,
}

fn main() {
    let flags = Flags::from_args();

    println!("Opening {}", flags.abst_input);
    let map = map_model::Map::new(&flags.abst_input, &map_model::Edits::new())
        .expect("Couldn't load map");
    // TODO could load savestate
    let control_map = control::ControlMap::new(&map);
    let mut sim = sim::Sim::new(&map, flags.rng_seed);

    if let Some(path) = flags.load_from {
        sim = abstutil::read_json(&path).expect("loading sim state failed");
        println!("Loaded {}", path);
    } else {
        // TODO need a notion of scenarios
        sim.seed_parked_cars(0.5);
        sim.seed_pedestrians(&map, 100);
        sim.start_many_parked_cars(&map, 100);
    }

    let mut benchmark = sim.start_benchmark();
    loop {
        sim.step(&map, &control_map);
        if sim.time.is_multiple_of_minute() {
            let speed = sim.measure_speed(&mut benchmark);
            println!("{0}, speed = {1:.2}x", sim.summary(), speed);
        }
        if let Some(ticks) = flags.save_at {
            if sim.time == sim::Tick::from_raw(ticks) {
                abstutil::write_json("sim_state", &sim).expect("Writing sim state failed");
                println!("Wrote sim_state at {}", sim.time);
            }
        }
    }
}

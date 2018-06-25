// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

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
}

fn main() {
    let flags = Flags::from_args();

    println!("Opening {}", flags.abst_input);
    let map = map_model::Map::new(&flags.abst_input).expect("Couldn't load map");
    // TODO could load savestate
    let control_map = control::ControlMap::new(&map);
    let mut sim = sim::straw_model::Sim::new(&map, flags.rng_seed);
    // TODO need a notion of scenarios
    sim.spawn_many_on_empty_roads(&map, 100000);

    let mut counter = 0;
    let mut benchmark = sim.start_benchmark();
    loop {
        counter += 1;
        sim.step(&map, &control_map);
        if counter % 1000 == 0 {
            let speed = sim.measure_speed(&mut benchmark);
            println!("{0}, speed = {1:.2}x", sim.summary(), speed);
        }
    }
}

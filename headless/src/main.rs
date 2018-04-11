// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate control;
extern crate geom;
extern crate map_model;
extern crate rand;
extern crate sim;
#[macro_use]
extern crate structopt;

use rand::SeedableRng;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "headless")]
struct Flags {
    /// ABST input to load
    #[structopt(name = "abst_input")]
    abst_input: String,

    /// Optional RNG seed
    #[structopt(long = "rng_seed")]
    rng_seed: Option<u32>,
}

fn main() {
    let flags = Flags::from_args();

    let mut rng = rand::weak_rng();
    if let Some(seed) = flags.rng_seed {
        rng.reseed([seed, seed + 1, seed + 2, seed + 3]);
    }

    println!("Opening {}", flags.abst_input);
    let data = map_model::load_pb(&flags.abst_input).expect("Couldn't load pb");
    let map = map_model::Map::new(&data);
    let geom_map = geom::GeomMap::new(&map);
    // TODO could load savestate
    let control_map = control::ControlMap::new(&map, &geom_map);
    let mut sim = sim::straw_model::Sim::new(&map, &geom_map);
    // TODO need a notion of scenarios
    sim.spawn_many_on_empty_roads(100000, &mut rng);

    let mut counter = 0;
    let mut benchmark = sim.start_benchmark();
    loop {
        counter += 1;
        sim.step(&geom_map, &map, &control_map, &mut rng);
        if counter % 1000 == 0 {
            let speed = sim.measure_speed(&mut benchmark);
            println!("{0}, speed = {1:.2}x", sim.summary(), speed);
        }
    }
}

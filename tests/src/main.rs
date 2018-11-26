extern crate abstutil;
extern crate convert_osm;
extern crate dimensioned;
extern crate gag;
extern crate geom;
extern crate map_model;
extern crate sim;
extern crate structopt;
extern crate yansi;

mod map_conversion;
mod parking;
mod physics;
mod runner;
mod sim_completion;
mod sim_determinism;
mod transit;

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "tests")]
struct Flags {
    /// Which tests to run?
    #[structopt(long = "filter", default_value = "All")]
    filter: runner::Filter,

    /// If specified, only run tests with names containing this substring.
    #[structopt(long = "test_names")]
    test_names: Option<String>,
}

fn main() {
    let flags = Flags::from_args();
    let mut t = runner::TestRunner::new(flags.filter, flags.test_names);

    map_conversion::run(t.suite("map_conversion"));
    parking::run(t.suite("parking"));
    physics::run(t.suite("physics"));
    sim_completion::run(t.suite("sim_completion"));
    sim_determinism::run(t.suite("sim_determinism"));
    transit::run(t.suite("transit"));

    t.done();
}

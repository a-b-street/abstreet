extern crate abstutil;
extern crate convert_osm;
extern crate dimensioned;
extern crate gag;
extern crate geom;
extern crate sim;
extern crate structopt;
extern crate yansi;

mod map_conversion;
mod physics;
mod runner;

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "tests")]
struct Flags {
    /// Which tests to run?
    #[structopt(long = "filter", default_value = "All")]
    filter: runner::Filter,
}

fn main() {
    let mut t = runner::TestRunner::new(Flags::from_args().filter);

    map_conversion::run(t.suite("map_conversion"));
    physics::run(t.suite("physics"));

    t.done();
}

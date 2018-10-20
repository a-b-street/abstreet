// TODO Bit clunky to live in the sim crate. This tool loads a raw map with edits, then saves it.
extern crate sim;
extern crate structopt;

use sim::SimFlags;
use structopt::StructOpt;

fn main() {
    let flags = SimFlags::from_args();
    let (map, _, _) = sim::load(flags, None);
    map.save();
}

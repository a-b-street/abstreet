extern crate abstutil;
extern crate log;
extern crate sim;
extern crate structopt;

use abstutil::{LogAdapter, Timer};
use log::LevelFilter;
use sim::SimFlags;
use structopt::StructOpt;

static LOG_ADAPTER: LogAdapter = LogAdapter;

fn main() {
    log::set_max_level(LevelFilter::Info);
    log::set_logger(&LOG_ADAPTER).unwrap();

    let flags = SimFlags::from_args();
    let mut timer = Timer::new(&format!(
        "precompute {} with {}",
        flags.load, flags.edits_name
    ));
    let (map, _, _) = sim::load(flags, None, &mut timer);
    timer.start("save map");
    map.save();
    timer.stop("save map");
    timer.done();
}

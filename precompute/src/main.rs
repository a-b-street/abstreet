extern crate abstutil;
extern crate log;
extern crate sim;
extern crate structopt;

use abstutil::LogAdapter;
use log::LevelFilter;
use sim::SimFlags;
use structopt::StructOpt;

static LOG_ADAPTER: LogAdapter = LogAdapter;

fn main() {
    log::set_max_level(LevelFilter::Info);
    log::set_logger(&LOG_ADAPTER).unwrap();

    let flags = SimFlags::from_args();
    let (map, _, _) = sim::load(flags, None);
    map.save();
}

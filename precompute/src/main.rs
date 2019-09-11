use abstutil::Timer;
use geom::Duration;
use map_model::Map;
use popdat::trips_to_scenario;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "precompute")]
struct Flags {
    /// Map
    #[structopt(name = "load")]
    load: String,

    /// Disable PSRC scenario generation for faster development
    #[structopt(name = "disable_psrc_scenarios")]
    disable_psrc_scenarios: bool,
}

fn main() {
    let flags = Flags::from_args();
    let mut timer = Timer::new(&format!("precompute {}", flags.load,));

    let map = Map::new(&flags.load, &mut timer).unwrap();
    timer.start("save map");
    map.save();
    timer.stop("save map");

    if !flags.disable_psrc_scenarios {
        trips_to_scenario(&map, Duration::ZERO, Duration::END_OF_DAY, &mut timer).save();
    }
}

//! Generates a scenario for a map using census data. Example call:
//!
//! cargo run --bin popdat -- --map=data/system/nyc/maps/lower_manhattan.bin --rng=42 \
//!   --scenario_name=monday

use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use abstutil::{CmdArgs, Timer};
use map_model::Map;

fn main() {
    let mut timer = Timer::new("generate scenario");
    let mut args = CmdArgs::new();
    let seed: u64 = args.required("--rng").parse().unwrap();
    let mut rng = XorShiftRng::seed_from_u64(seed);
    let map = Map::new(args.required("--map"), &mut timer);
    let scenario_name = args.required("--scenario_name");
    args.done();

    timer.start("generate");
    let scenario =
        popdat::generate_scenario(&scenario_name, popdat::Config::default(), &map, &mut rng)
            .unwrap();
    timer.stop("generate");
    scenario.save();
}

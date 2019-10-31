use abstutil::{CmdArgs, Timer};
use map_model::Map;
use popdat::trips_to_scenario;

fn main() {
    let mut args = CmdArgs::new();
    let load = args.required_free();
    let disable_psrc_scenarios = args.enabled("--disable_psrc_scenarios");
    let use_fixes = !args.enabled("--nofixes");
    args.done();

    let mut timer = Timer::new(&format!("precompute {}", load));

    let map = Map::new(&load, use_fixes, &mut timer).unwrap();
    timer.start("save map");
    map.save();
    timer.stop("save map");

    if !disable_psrc_scenarios {
        trips_to_scenario(&map, &mut timer).save();
    }
}

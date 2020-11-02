use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use abstutil::{CmdArgs, Timer};
use map_model::Map;
use sim::ScenarioGenerator;

fn main() {
    let mut args = CmdArgs::new();
    let seed: u64 = args.required("--rng").parse().unwrap();
    let mut rng = XorShiftRng::seed_from_u64(seed);
    let map = Map::new(args.required("--map"), &mut Timer::throwaway());
    let scenario_name = args.required("--scenario_name");
    args.done();

    let mut scenario =
        ScenarioGenerator::proletariat_robot(&map, &mut rng, &mut Timer::throwaway());
    scenario.scenario_name = scenario_name;
    scenario.save();
}

use rand::SeedableRng;
use rand_xorshift::XorShiftRng;

use abstutil::{CmdArgs, Timer};
use map_model::Map;
use sim::ScenarioGenerator;

fn main() {
    let mut args = CmdArgs::new();
    let seed: u8 = args.required("--rng").parse().unwrap();
    let mut rng = XorShiftRng::from_seed([seed; 16]);
    let map = Map::new(args.required("--map"), &mut Timer::throwaway());
    args.done();

    let scenario = ScenarioGenerator::proletariat_robot(&map, &mut rng, &mut Timer::throwaway());
    println!("{}", abstutil::to_json(&scenario));
}

// TODO Can we list example invocations in the help text?
// dump-json data/system/us/seattle/maps/montlake.bin
// dump-json data/system/us/seattle/scenarios/montlake/weekday.bin
// random-scenario --map data/system/us/seattle/maps/montlake.bin --rng-seed 42 --scenario-name thursday

use structopt::StructOpt;

use abstutil::Timer;

#[derive(StructOpt)]
#[structopt(name = "abcli", about = "The A/B Street multi-tool")]
enum Command {
    /// Print a binary map or scenario file as JSON
    DumpJSON {
        #[structopt()]
        path: String,
    },
    /// Generates a random scenario using the proletariat robot travel demand model
    RandomScenario {
        /// A seed for generating random numbers
        #[structopt(long)]
        rng_seed: u64,
        /// The path to a map to generate a scenario for
        #[structopt(long)]
        map: String,
        /// The name of the scenario to generate
        #[structopt(long)]
        scenario_name: String,
    },
}

fn main() {
    // Short implementations can stay in this file, but please split larger subcommands to their
    // own module.
    match Command::from_args() {
        Command::DumpJSON { path } => dump_json(path),
        Command::RandomScenario {
            rng_seed,
            map,
            scenario_name,
        } => random_scenario(rng_seed, map, scenario_name),
    }
}

fn dump_json(path: String) {
    if path.contains("/maps/") {
        let map = map_model::Map::load_synchronously(path, &mut Timer::throwaway());
        println!("{}", abstutil::to_json(&map));
    } else if path.contains("/scenarios/") {
        let scenario: sim::Scenario = abstio::read_binary(path, &mut Timer::throwaway());
        println!("{}", abstutil::to_json(&scenario));
    } else {
        panic!(
            "Don't know how to dump JSON for {}. Only maps and scenarios are supported.",
            path
        );
    }
}

fn random_scenario(rng_seed: u64, map: String, scenario_name: String) {
    use rand::SeedableRng;
    use rand_xorshift::XorShiftRng;

    let mut rng = XorShiftRng::seed_from_u64(rng_seed);
    let map = map_model::Map::load_synchronously(map, &mut Timer::throwaway());
    let mut scenario =
        sim::ScenarioGenerator::proletariat_robot(&map, &mut rng, &mut Timer::throwaway());
    scenario.scenario_name = scenario_name;
    scenario.save();
    println!(
        "Wrote {}",
        abstio::path_scenario(&scenario.map_name, &scenario.scenario_name)
    );
}

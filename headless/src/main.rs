use abstutil::{prettyprint_usize, CmdArgs, Timer};
use map_model::Map;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use sim::{Scenario, Sim, SimFlags};

// This is specialized to experiment with running the pandemic model over long time periods.
// Original functionality for profiling and debugging gridlock have been removed.

fn main() {
    let mut args = CmdArgs::new();
    let num_days = args
        .optional_parse("--days", |s| s.parse::<usize>())
        .unwrap_or(1);
    args.done();

    let mut sim_flags = SimFlags::synthetic_test("montlake", "pandemic");
    sim_flags.opts.enable_pandemic_model = Some(XorShiftRng::from_seed([42; 16]));
    let mut timer = Timer::new("setup headless");
    let (map, mut sim, mut rng) = sim_flags.load(&mut timer);

    let base_scenario: Scenario = abstutil::read_binary(
        abstutil::path_scenario(map.get_name(), "weekday"),
        &mut timer,
    );
    // TODO Hack: avoid leaking parking spots
    base_scenario
        .repeat_days(num_days, true)
        .instantiate(&mut sim, &map, &mut rng, &mut timer);
    timer.done();

    run_experiment(&map, &mut sim);
}

fn run_experiment(map: &Map, sim: &mut Sim) {
    let timer = Timer::new("run sim until done");
    sim.run_until_done(
        &map,
        |sim, _map| {
            // This'll run every 30 sim seconds
            println!(
                "At {}, {} infected",
                sim.time(),
                prettyprint_usize(sim.get_pandemic_model().unwrap().count_infected())
            );
        },
        None,
    );
    timer.done();
    println!("Done at {}", sim.time());
}

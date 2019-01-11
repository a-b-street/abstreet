use crate::runner::TestRunner;
use abstutil::Timer;
use sim;

pub fn run(t: &mut TestRunner) {
    t.run_slow(
        "small_spawn_completes",
        Box::new(|h| {
            let (map, mut sim) = sim::load(
                sim::SimFlags::for_test("aorta_model_completes"),
                Some(sim::Tick::from_seconds(30)),
                &mut Timer::new("setup test"),
            );
            sim.small_spawn(&map);
            h.setup_done(&sim);
            sim.run_until_done(&map, Box::new(|_sim| {}), Some(sim::Tick::from_minutes(60)));
        }),
    );
}

// TODO other tests (not completion) to add:
// - different behavior (stopping or not) at stop signs

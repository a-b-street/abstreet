use abstutil::Timer;
use runner::TestRunner;
use sim;

pub fn run(t: &mut TestRunner) {
    t.run_slow(
        "small_spawn_completes",
        Box::new(|_| {
            let (map, control_map, mut sim) = sim::load(
                sim::SimFlags::for_test("aorta_model_completes"),
                Some(sim::Tick::from_seconds(30)),
                &mut Timer::new("setup test"),
            );
            sim.small_spawn(&map);
            sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
        }),
    );
}

// TODO other tests (not completion) to add:
// - different behavior (stopping or not) at stop signs

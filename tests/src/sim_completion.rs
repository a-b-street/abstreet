use crate::runner::TestRunner;
use abstutil::Timer;
use geom::Duration;
use sim;

pub fn run(t: &mut TestRunner) {
    t.run_slow("small_spawn_completes", |h| {
        let (map, mut sim) = sim::load(
            sim::SimFlags::for_test("aorta_model_completes"),
            Some(Duration::seconds(30.0)),
            &mut Timer::throwaway(),
        );
        sim.small_spawn(&map);
        h.setup_done(&sim);
        sim.run_until_done(&map, |_| {}, Some(Duration::minutes(60)));
    });
}

// TODO other tests (not completion) to add:
// - different behavior (stopping or not) at stop signs

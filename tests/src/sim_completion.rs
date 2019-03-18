use crate::runner::TestRunner;
use abstutil::Timer;
use geom::Duration;
use sim::{Scenario, SimFlags};

pub fn run(t: &mut TestRunner) {
    t.run_slow("small_spawn_completes", |h| {
        let (map, mut sim, mut rng) = SimFlags::for_test("aorta_model_completes")
            .load(Some(Duration::seconds(30.0)), &mut Timer::throwaway());
        Scenario::small_run(&map).instantiate(&mut sim, &map, &mut rng, &mut Timer::throwaway());
        h.setup_done(&sim);
        sim.just_run_until_done(&map, Some(Duration::minutes(70)));
    });
}

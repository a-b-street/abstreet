extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn aorta_model_completes() {
    let (map, control_map, mut sim) = sim::load(
        sim::SimFlags::for_test("aorta_model_completes"),
        Some(sim::Tick::from_seconds(30)),
        &mut abstutil::Timer::new("setup test"),
    );
    sim.small_spawn(&map);
    sim.run_until_done(&map, &control_map, Box::new(|_sim| {}));
}

// TODO other tests (not completion) to add:
// - different behavior (stopping or not) at stop signs

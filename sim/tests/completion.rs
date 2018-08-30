extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn aorta_model_completes() {
    let (map, _, control_map, mut sim) = sim::init::load(
        "../data/small.abst".to_string(),
        "aorta_model_completes".to_string(),
        Some(42),
        Some(sim::Tick::from_seconds(30)),
    );
    sim::init::small_spawn(&mut sim, &map);
    sim::init::run_until_done(&mut sim, &map, &control_map, Box::new(|_sim| {}));
}

// TODO other tests (not completion) to add:
// - different behavior (stopping or not) at stop signs

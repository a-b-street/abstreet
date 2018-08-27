extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn aorta_model_completes() {
    // This assumes this map has been built
    let input = "../data/small.abst";
    let rng_seed = 42;
    let spawn_count = 100;

    let map = map_model::Map::new(input, &map_model::Edits::new()).expect("Couldn't load map");
    let control_map = control::ControlMap::new(&map);

    // TODO need https://github.com/rust-lang/rfcs/issues/1743 to be less repetitive :(
    let mut sim = sim::Sim::new(&map, "aorta_model_completes".to_string(), Some(rng_seed));
    sim.seed_parked_cars(0.5);
    sim.seed_walking_trips(&map, spawn_count);
    sim.seed_driving_trips(&map, spawn_count);

    loop {
        sim.step(&map, &control_map);
        if sim.time.is_multiple_of_minute() {
            println!("{}", sim.summary());
        }
        if sim.is_done() {
            break;
        }
    }
}

// TODO other tests (not completion) to add:
// - different behavior (stopping or not) at stop signs

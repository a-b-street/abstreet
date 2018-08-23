extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn serialization() {
    // This assumes this map has been built
    let input = "../data/small.abst";
    let rng_seed = 42;
    let spawn_count = 10;

    let map = map_model::Map::new(input, &map_model::Edits::new()).expect("Couldn't load map");

    let mut sim = sim::Sim::new(&map, Some(rng_seed));
    sim.seed_parked_cars(0.5);
    sim.seed_walking_trips(&map, spawn_count);
    sim.seed_driving_trips(&map, spawn_count);

    // Does savestating produce the same string?
    let save1 = abstutil::to_json(&sim);
    let save2 = abstutil::to_json(&sim);
    assert_eq!(save1, save2);
}

#[test]
fn from_scratch() {
    // This assumes this map has been built
    let input = "../data/small.abst";
    let rng_seed = 42;
    let spawn_count = 100;

    println!("Creating two simulations");
    let map = map_model::Map::new(input, &map_model::Edits::new()).expect("Couldn't load map");
    let control_map = control::ControlMap::new(&map);

    let mut sim1 = sim::Sim::new(&map, Some(rng_seed));
    let mut sim2 = sim::Sim::new(&map, Some(rng_seed));
    sim1.seed_parked_cars(0.5);
    sim1.seed_walking_trips(&map, spawn_count);
    sim1.seed_driving_trips(&map, spawn_count);
    sim2.seed_parked_cars(0.5);
    sim2.seed_walking_trips(&map, spawn_count);
    sim2.seed_driving_trips(&map, spawn_count);

    for _ in 1..600 {
        if sim1 != sim2 {
            // TODO write to temporary files somewhere
            // TODO need to sort dicts in json output to compare
            abstutil::write_json("sim1_state.json", &sim1).unwrap();
            abstutil::write_json("sim2_state.json", &sim2).unwrap();
            panic!(
                "sim state differs at {}. compare sim1_state.json and sim2_state.json",
                sim1.time
            );
        }
        sim1.step(&map, &control_map);
        sim2.step(&map, &control_map);
    }
}

#[test]
fn with_savestating() {
    // This assumes this map has been built
    let input = "../data/small.abst";
    let rng_seed = 42;
    let spawn_count = 100;

    println!("Creating two simulations");
    let map = map_model::Map::new(input, &map_model::Edits::new()).expect("Couldn't load map");
    let control_map = control::ControlMap::new(&map);

    let mut sim1 = sim::Sim::new(&map, Some(rng_seed));
    let mut sim2 = sim::Sim::new(&map, Some(rng_seed));
    sim1.seed_parked_cars(0.5);
    sim1.seed_walking_trips(&map, spawn_count);
    sim1.seed_driving_trips(&map, spawn_count);
    sim2.seed_parked_cars(0.5);
    sim2.seed_walking_trips(&map, spawn_count);
    sim2.seed_driving_trips(&map, spawn_count);

    for _ in 1..600 {
        sim1.step(&map, &control_map);
        sim2.step(&map, &control_map);
    }

    if sim1 != sim2 {
        abstutil::write_json("sim1_state.json", &sim1).unwrap();
        abstutil::write_json("sim2_state.json", &sim2).unwrap();
        panic!(
            "sim state differs at {}. compare sim1_state.json and sim2_state.json",
            sim1.time
        );
    }

    abstutil::write_json("sim1_savestate.json", &sim1).unwrap();

    for _ in 1..60 {
        sim1.step(&map, &control_map);
    }

    if sim1 == sim2 {
        abstutil::write_json("sim1_state.json", &sim1).unwrap();
        abstutil::write_json("sim2_state.json", &sim2).unwrap();
        panic!(
            "sim state unexpectedly the same at {}. compare sim1_state.json and sim2_state.json",
            sim1.time
        );
    }

    let sim3: sim::Sim = abstutil::read_json("sim1_savestate.json").unwrap();
    if sim3 != sim2 {
        abstutil::write_json("sim3_state.json", &sim3).unwrap();
        abstutil::write_json("sim2_state.json", &sim2).unwrap();
        panic!(
            "sim state differs at {}. compare sim3_state.json and sim2_state.json",
            sim1.time
        );
    }

    std::fs::remove_file("sim1_savestate.json").unwrap();
}

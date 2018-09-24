extern crate abstutil;
extern crate control;
extern crate map_model;
extern crate sim;

#[test]
fn serialization() {
    let (map, _, _, mut sim) = sim::load(
        "../data/maps/small.abst".to_string(),
        "serialization".to_string(),
        Some(42),
        None,
    );
    sim.small_spawn(&map);

    // Does savestating produce the same string?
    let save1 = abstutil::to_json(&sim);
    let save2 = abstutil::to_json(&sim);
    assert_eq!(save1, save2);
}

#[test]
fn from_scratch() {
    println!("Creating two simulations");
    let (map, _, control_map, mut sim1) = sim::load(
        "../data/maps/small.abst".to_string(),
        "from_scratch_1".to_string(),
        Some(42),
        None,
    );
    let mut sim2 = sim::Sim::new(&map, "from_scratch_2".to_string(), Some(42), None);
    sim1.small_spawn(&map);
    sim2.small_spawn(&map);

    for _ in 1..600 {
        if sim1 != sim2 {
            // TODO need to sort dicts in json output to compare
            panic!(
                "sim state differs between {} and {}",
                sim1.save(),
                sim2.save()
            );
        }
        sim1.step(&map, &control_map);
        sim2.step(&map, &control_map);
    }
}

#[test]
fn with_savestating() {
    println!("Creating two simulations");
    let (map, _, control_map, mut sim1) = sim::load(
        "../data/maps/small.abst".to_string(),
        "with_savestating_1".to_string(),
        Some(42),
        None,
    );
    let mut sim2 = sim::Sim::new(&map, "with_savestating_2".to_string(), Some(42), None);
    sim1.small_spawn(&map);
    sim2.small_spawn(&map);

    for _ in 1..600 {
        sim1.step(&map, &control_map);
        sim2.step(&map, &control_map);
    }

    if sim1 != sim2 {
        panic!(
            "sim state differs between {} and {}",
            sim1.save(),
            sim2.save()
        );
    }

    let sim1_save = sim1.save();

    for _ in 1..60 {
        sim1.step(&map, &control_map);
    }

    if sim1 == sim2 {
        panic!(
            "sim state unexpectly the same -- {} and {}",
            sim1.save(),
            sim2.save()
        );
    }

    let sim3: sim::Sim =
        sim::Sim::load(sim1_save.clone(), "with_savestating_3".to_string()).unwrap();
    if sim3 != sim2 {
        panic!(
            "sim state differs between {} and {}",
            sim3.save(),
            sim2.save()
        );
    }

    std::fs::remove_file(sim1_save).unwrap();
}

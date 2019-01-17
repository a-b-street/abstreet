use crate::runner::TestRunner;
use abstutil::Timer;
use sim;

pub fn run(t: &mut TestRunner) {
    t.run_slow("serialization", |_| {
        let (map, mut sim) = sim::load(
            sim::SimFlags::for_test("serialization"),
            None,
            &mut Timer::new("setup test"),
        );
        sim.small_spawn(&map);

        // Does savestating produce the same string?
        let save1 = abstutil::to_json(&sim);
        let save2 = abstutil::to_json(&sim);
        assert_eq!(save1, save2);
    });

    t.run_slow("from_scratch", |_| {
        println!("Creating two simulations");
        let (map, mut sim1) = sim::load(
            sim::SimFlags::for_test("from_scratch_1"),
            None,
            &mut Timer::new("setup test"),
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
            sim1.step(&map);
            sim2.step(&map);
        }
    });

    t.run_slow("with_savestating", |_| {
        println!("Creating two simulations");
        let (map, mut sim1) = sim::load(
            sim::SimFlags::for_test("with_savestating_1"),
            None,
            &mut Timer::new("setup test"),
        );
        let mut sim2 = sim::Sim::new(&map, "with_savestating_2".to_string(), Some(42), None);
        sim1.small_spawn(&map);
        sim2.small_spawn(&map);

        for _ in 1..600 {
            sim1.step(&map);
            sim2.step(&map);
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
            sim1.step(&map);
        }

        if sim1 == sim2 {
            panic!(
                "sim state unexpectly the same -- {} and {}",
                sim1.save(),
                sim2.save()
            );
        }

        let sim3: sim::Sim =
            sim::Sim::load_savestate(sim1_save.clone(), Some("with_savestating_3".to_string()))
                .unwrap();
        if sim3 != sim2 {
            panic!(
                "sim state differs between {} and {}",
                sim3.save(),
                sim2.save()
            );
        }

        std::fs::remove_file(sim1_save).unwrap();
    });
}

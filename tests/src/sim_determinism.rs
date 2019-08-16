use crate::runner::TestRunner;
use abstutil::Timer;
use geom::Duration;
use sim::{Scenario, Sim, SimFlags, SimOptions};

pub fn run(t: &mut TestRunner) {
    t.run_slow("serialization", |_| {
        let (map, mut sim, mut rng) =
            SimFlags::for_test("serialization").load(None, &mut Timer::throwaway());
        Scenario::small_run(&map).instantiate(&mut sim, &map, &mut rng, &mut Timer::throwaway());

        // Does savestating produce the same string?
        let save1 = abstutil::to_json(&sim);
        let save2 = abstutil::to_json(&sim);
        assert_eq!(save1, save2);
    });

    t.run_slow("from_scratch", |_| {
        println!("Creating two simulations");
        let flags = SimFlags::for_test("from_scratch_1");
        let (map, mut sim1, _) = flags.load(None, &mut Timer::throwaway());
        let mut sim2 = Sim::new(&map, SimOptions::new("from_scratch_2"));
        Scenario::small_run(&map).instantiate(
            &mut sim1,
            &map,
            &mut flags.make_rng(),
            &mut Timer::throwaway(),
        );
        Scenario::small_run(&map).instantiate(
            &mut sim2,
            &map,
            &mut flags.make_rng(),
            &mut Timer::throwaway(),
        );

        let dt = Duration::seconds(0.1);
        for _ in 1..600 {
            if sim1 != sim2 {
                // TODO need to sort dicts in json output to compare
                panic!(
                    "sim state differs between {} and {}",
                    sim1.save(),
                    sim2.save()
                );
            }
            sim1.step(&map, dt);
            sim2.step(&map, dt);
        }
    });

    t.run_slow("with_savestating", |_| {
        println!("Creating two simulations");
        let flags = SimFlags::for_test("with_savestating_1");
        let (map, mut sim1, _) = flags.load(None, &mut Timer::throwaway());
        let mut sim2 = Sim::new(&map, SimOptions::new("with_savestating_2"));
        Scenario::small_run(&map).instantiate(
            &mut sim1,
            &map,
            &mut flags.make_rng(),
            &mut Timer::throwaway(),
        );
        Scenario::small_run(&map).instantiate(
            &mut sim2,
            &map,
            &mut flags.make_rng(),
            &mut Timer::throwaway(),
        );

        sim1.step(&map, Duration::minutes(10));
        sim2.step(&map, Duration::minutes(10));

        if sim1 != sim2 {
            panic!(
                "sim state differs between {} and {}",
                sim1.save(),
                sim2.save()
            );
        }

        let sim1_save = sim1.save();

        sim1.step(&map, Duration::seconds(30.0));

        if sim1 == sim2 {
            panic!(
                "sim state unexpectly the same -- {} and {}",
                sim1.save(),
                sim2.save()
            );
        }

        let mut sim3: Sim =
            Sim::load_savestate(sim1_save.clone(), &mut Timer::throwaway()).unwrap();
        sim3.set_name("with_savestating_3".to_string());
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

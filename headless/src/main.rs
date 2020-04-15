use abstutil::{CmdArgs, Timer};
use geom::Time;
use sim::{GetDrawAgents, ScenarioGenerator, SimFlags};

fn main() {
    let mut args = CmdArgs::new();
    let sim_flags = SimFlags::from_args(&mut args);
    let save_at = args.optional_parse("--save_at", Time::parse);
    let num_agents = args.optional_parse("--num_agents", |s| s.parse::<usize>());
    let enable_profiler = args.enabled("--enable_profiler");
    // Every 0.1s, pretend to draw everything to make sure there are no bugs.
    let paranoia = args.enabled("--paranoia");
    args.done();

    let mut timer = Timer::new("setup headless");
    let (map, mut sim, mut rng) = sim_flags.load(&mut timer);

    // TODO not the ideal way to distinguish what thing we loaded
    if sim_flags.load.starts_with(&abstutil::path_all_raw_maps())
        || sim_flags.load.starts_with(&abstutil::path_all_maps())
    {
        let s = if let Some(n) = num_agents {
            ScenarioGenerator::scaled_run(n)
        } else {
            ScenarioGenerator::small_run(&map)
        }
        .generate(&map, &mut rng, &mut timer);
        s.instantiate(&mut sim, &map, &mut rng, &mut timer);
    }
    timer.done();

    if enable_profiler {
        #[cfg(feature = "profiler")]
        {
            cpuprofiler::PROFILER
                .lock()
                .unwrap()
                .start("./profile")
                .unwrap();
        }
    }
    let timer = Timer::new("run sim until done");
    sim.run_until_done(
        &map,
        move |sim, map| {
            // TODO We want to savestate at the end of this time; this'll happen at the beginning.
            if Some(sim.time()) == save_at {
                sim.save();
                // Some simulations run for a really long time, just do this.
                if enable_profiler {
                    #[cfg(feature = "profiler")]
                    {
                        cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                    }
                }
            }
            if paranoia {
                sim.get_all_draw_cars(map);
            }
        },
        None,
    );
    timer.done();
    println!("Done at {}", sim.time());
    if enable_profiler && save_at.is_none() {
        #[cfg(feature = "profiler")]
        {
            cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
        }
    }
}

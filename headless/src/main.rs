use abstutil::Timer;
use geom::Duration;
use sim::{GetDrawAgents, Scenario, SimFlags};
use std::path::Path;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "headless")]
struct Flags {
    #[structopt(flatten)]
    sim_flags: SimFlags,

    /// Optional time to savestate
    #[structopt(long = "save_at")]
    save_at: Option<String>,

    /// Number of agents to generate. If unspecified, trips to/from borders will be included.
    #[structopt(long = "num_agents")]
    num_agents: Option<usize>,

    /// Enable cpuprofiler?
    #[structopt(long = "enable_profiler")]
    enable_profiler: bool,

    /// Every 0.1s, pretend to draw everything to make sure there are no bugs.
    #[structopt(long = "paranoia")]
    paranoia: bool,
}

fn main() {
    let flags = Flags::from_args();

    let save_at = if let Some(ref time_str) = flags.save_at {
        if let Some(t) = Duration::parse(time_str) {
            Some(t)
        } else {
            panic!("Couldn't parse time {}", time_str);
        }
    } else {
        None
    };

    // TODO not the ideal way to distinguish what thing we loaded
    let load = flags.sim_flags.load.clone();
    let mut timer = Timer::new("setup headless");
    let (map, mut sim, mut rng) = flags.sim_flags.load(None, &mut timer);

    if load.starts_with(Path::new("../data/raw_maps/"))
        || load.starts_with(Path::new("../data/maps/"))
    {
        let s = if let Some(n) = flags.num_agents {
            Scenario::scaled_run(&map, n)
        } else {
            Scenario::small_run(&map)
        };
        s.instantiate(&mut sim, &map, &mut rng, &mut timer);
    }
    timer.done();

    if flags.enable_profiler {
        cpuprofiler::PROFILER
            .lock()
            .unwrap()
            .start("./profile")
            .unwrap();
    }
    let enable_profiler = flags.enable_profiler;
    let paranoia = flags.paranoia;
    let timer = Timer::new("run sim until done");
    sim.run_until_done(
        &map,
        move |sim, map| {
            // TODO We want to savestate at the end of this time; this'll happen at the beginning.
            if Some(sim.time()) == save_at {
                sim.save();
                // Some simulations run for a really long time, just do this.
                if enable_profiler {
                    cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                }
            }
            if paranoia {
                sim.get_all_draw_cars(map);
            }
        },
        None,
    );
    timer.done();
    println!("{:?}", sim.get_score());
    if flags.enable_profiler && save_at.is_none() {
        cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
    }
}

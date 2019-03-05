use abstutil::Timer;
use geom::Duration;
use sim::{Scenario, SimFlags};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "headless")]
struct Flags {
    #[structopt(flatten)]
    sim_flags: SimFlags,

    /// Optional time to savestate
    #[structopt(long = "save_at")]
    save_at: Option<String>,

    /// Enable cpuprofiler?
    #[structopt(long = "enable_profiler")]
    pub enable_profiler: bool,
}

fn main() {
    let flags = Flags::from_args();

    // TODO not the ideal way to distinguish what thing we loaded
    let load = flags.sim_flags.load.clone();
    let mut timer = Timer::new("setup headless");
    let (map, mut sim, mut rng) = flags.sim_flags.load(None, &mut timer);

    if load.contains("data/raw_maps/") || load.contains("data/maps/") {
        Scenario::small_run(&map).instantiate(&mut sim, &map, &mut rng, &mut timer);
    }
    timer.done();

    let save_at = if let Some(ref time_str) = flags.save_at {
        if let Some(t) = Duration::parse(time_str) {
            Some(t)
        } else {
            panic!("Couldn't parse time {}", time_str);
        }
    } else {
        None
    };

    if flags.enable_profiler {
        cpuprofiler::PROFILER
            .lock()
            .unwrap()
            .start("./profile")
            .unwrap();
    }
    let enable_profiler = flags.enable_profiler;
    let timer = Timer::new("run sim until done");
    sim.run_until_done(
        &map,
        move |sim| {
            if Some(sim.time()) == save_at {
                sim.save();
                // Some simulatiosn run for a really long time, just do this.
                if enable_profiler {
                    cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
                }
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

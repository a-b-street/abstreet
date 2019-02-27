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
}

fn main() {
    let flags = Flags::from_args();

    // TODO not the ideal way to distinguish what thing we loaded
    let load = flags.sim_flags.load.clone();
    let mut timer = Timer::new("setup headless");
    let (map, mut sim, mut rng) = flags
        .sim_flags
        .load(Some(Duration::seconds(30.0)), &mut timer);

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

    /*cpuprofiler::PROFILER
    .lock()
    .unwrap()
    .start("./profile")
    .unwrap();*/
    sim.run_until_done(
        &map,
        move |sim| {
            if Some(sim.time()) == save_at {
                sim.save();
                // Some simulatiosn run for a really long time, just do this.
                //cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
            }
        },
        None,
    );
    //sim::save_backtraces("call_graph.json");
    println!("{:?}", sim.get_score());
}

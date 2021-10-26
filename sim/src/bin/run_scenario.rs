//! A simple tool that just runs a simulation for the specified number of hours. Use for profiling
//! and benchmarking.

use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "run_scenario", about = "Simulates a scenario")]
struct Args {
    /// Pressing Control+C will interrupt and savestate.
    #[structopt(long)]
    interruptible: bool,
    /// How many hours to simulate.
    #[structopt(long)]
    hours: usize,
    #[structopt(flatten)]
    flags: sim::SimFlags,
}

fn main() {
    abstutil::logger::setup();
    let mut args = Args::from_args();
    args.flags.initialize();
    let hours = geom::Duration::hours(args.hours);
    let (mut map, mut sim, _) = args
        .flags
        .load_synchronously(&mut abstutil::Timer::new("setup"));

    if args.interruptible {
        // Pressing ^C will savestate. This needs a more complex loop to check for the interrupt.
        // This is guarded by the --interruptible flag to keep the benchmarking case simple.
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .unwrap();

        let start = instant::Instant::now();
        let goal_time = geom::Time::START_OF_DAY + hours;
        while running.load(Ordering::SeqCst) {
            println!(
                "After {}, the sim is at {}. {} live agents",
                geom::Duration::realtime_elapsed(start),
                sim.time(),
                abstutil::prettyprint_usize(sim.active_agents().len())
            );
            sim.time_limited_step(
                &map,
                goal_time - sim.time(),
                geom::Duration::seconds(1.0),
                &mut None,
            );
            if sim.time() == goal_time {
                return;
            }
        }
        println!("\n\nInterrupting at {}", sim.time());
        sim.save();
        for x in sim.describe_internal_stats() {
            println!("{}", x);
        }
    } else {
        sim.timed_step(
            &mut map,
            hours,
            &mut None,
            &mut abstutil::Timer::new("run simulation"),
        );
    }
}

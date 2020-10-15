//! A simple tool that just runs a simulation for the specified number of hours. Use for profiling
//! and benchmarking.

fn main() {
    let mut args = abstutil::CmdArgs::new();
    let hours = geom::Duration::hours(args.required("--hours").parse::<usize>().unwrap());
    let (mut map, mut sim, _) =
        sim::SimFlags::from_args(&mut args).load(&mut abstutil::Timer::new("setup"));
    args.done();

    sim.timed_step(
        &mut map,
        hours,
        &mut None,
        &mut abstutil::Timer::new("run simulation"),
    );
}

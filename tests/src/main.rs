mod geom;
mod map_conversion;
mod parking;
mod runner;
mod sim_completion;
mod sim_determinism;
mod transit;
mod trips;

use abstutil::CmdArgs;

fn main() {
    let mut args = CmdArgs::new();
    let flags = runner::Flags {
        filter: match args.optional("--filter") {
            Some(x) => match x.as_str() {
                "All" => runner::Filter::All,
                "Slow" => runner::Filter::Slow,
                "Fast" => runner::Filter::Fast,
                _ => panic!("Bad --filter={}", x),
            },
            None => runner::Filter::All,
        },
        test_names: args.optional("--test_names"),
        keep_output: args.enabled("--keep_output"),
        clickable_links: args.enabled("--clickable_links"),
    };
    args.done();

    let mut t = runner::TestRunner::new(flags);

    geom::run(t.suite("geom"));
    map_conversion::run(t.suite("map_conversion"));
    parking::run(t.suite("parking"));
    sim_completion::run(t.suite("sim_completion"));
    sim_determinism::run(t.suite("sim_determinism"));
    transit::run(t.suite("transit"));
    trips::run(t.suite("trips"));

    t.done();
}

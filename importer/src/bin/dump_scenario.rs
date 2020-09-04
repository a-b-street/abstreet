use abstutil::{CmdArgs, Timer};
use sim::Scenario;

fn main() {
    let mut args = CmdArgs::new();
    let scenario: Scenario = abstutil::read_binary(args.required_free(), &mut Timer::throwaway());
    println!("{}", abstutil::to_json(&scenario));
    args.done();
}

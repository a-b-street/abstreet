use serde::Deserialize;

use abstutil::{prettyprint_usize, CmdArgs, Timer};
use map_model::Map;
use sim::{ExternalPerson, Scenario};

fn main() {
    let mut args = CmdArgs::new();
    let map = args.required("--map");
    let input = args.required("--input");
    let skip_problems = args.enabled("--skip_problems");
    args.done();

    let mut timer = Timer::new("import traffic demand data");
    let map = Map::new(map, &mut timer);
    let input: Input = abstio::read_json(input, &mut timer);

    let mut s = Scenario::empty(&map, &input.scenario_name);
    // Include all buses/trains
    s.only_seed_buses = None;
    let orig_num = input.people.len();
    s.people = ExternalPerson::import(&map, input.people, skip_problems).unwrap();
    // Always clean up people with no-op trips (going between the same buildings)
    s = s.remove_weird_schedules();
    println!(
        "Imported {}/{} people",
        prettyprint_usize(s.people.len()),
        prettyprint_usize(orig_num)
    );
    s.save();
}

#[derive(Deserialize)]
struct Input {
    scenario_name: String,
    people: Vec<ExternalPerson>,
}

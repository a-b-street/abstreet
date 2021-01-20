use serde::Deserialize;

use abstutil::{CmdArgs, Timer};
use map_model::Map;
use sim::{ExternalPerson, Scenario};

fn main() {
    let mut args = CmdArgs::new();
    let map = args.required("--map");
    let input = args.required("--input");
    args.done();

    let mut timer = Timer::new("import traffic demand data");
    let map = Map::new(map, &mut timer);
    let input: Input = abstio::read_json(input, &mut timer);

    let mut s = Scenario::empty(&map, &input.scenario_name);
    // Include all buses/trains
    s.only_seed_buses = None;
    s.people = ExternalPerson::import(&map, input.people).unwrap();
    s.save();
}

#[derive(Deserialize)]
struct Input {
    scenario_name: String,
    people: Vec<ExternalPerson>,
}

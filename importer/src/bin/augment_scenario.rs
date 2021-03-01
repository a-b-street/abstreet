//! A tool to modify each person's schedule from an existing scenario in various ways.
//!
//! `--add_return_trips`: For people with only a single trip before noon, add a return trip back
//!                       home sometime in the evening.
//! `--add_lunch_trips`: After the last trip somebody takes before noon, insert a round-trip to a
//!                      nearby cafe or restaurant.
//!
//! These tools aren't very smart about detecting if a scenario already has these extra trips added
//! in; be careful about running this on the correct input. It modifies the given `--input` binary
//! scenario in-place.

use abstutil::{CmdArgs, Timer};
use map_model::Map;
use sim::Scenario;

fn main() {
    let mut args = CmdArgs::new();
    let input = args.required("--input");
    let should_add_return_trips = args.enabled("--add_return_trips");
    let should_add_lunch_trips = args.enabled("--add_lunch_trips");
    args.done();

    let mut timer = Timer::new("augment scenario");

    let mut scenario: Scenario = abstio::must_read_object(input, &mut timer);
    let map = Map::new(scenario.map_name.path(), &mut timer);

    if should_add_return_trips {
        add_return_trips(&mut scenario, &map);
    }
    if should_add_lunch_trips {
        add_lunch_trips(&mut scenario, &map, &mut timer);
    }

    scenario.save();
}

fn add_return_trips(scenario: &mut Scenario, map: &Map) {}

fn add_lunch_trips(scenario: &mut Scenario, map: &Map, timer: &mut Timer) {}

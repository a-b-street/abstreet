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

use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use abstutil::{CmdArgs, Timer};
use geom::Duration;
use map_model::Map;
use sim::{IndividTrip, Scenario, TripPurpose};

fn main() {
    let mut args = CmdArgs::new();
    let input = args.required("--input");
    let should_add_return_trips = args.enabled("--add_return_trips");
    let should_add_lunch_trips = args.enabled("--add_lunch_trips");
    let rng_seed: u64 = args
        .optional_parse("--rng_seed", |s| s.parse())
        .unwrap_or(42);
    args.done();

    let mut rng = XorShiftRng::seed_from_u64(rng_seed);
    let mut timer = Timer::new("augment scenario");

    let mut scenario: Scenario = abstio::must_read_object(input, &mut timer);
    let map = Map::new(scenario.map_name.path(), &mut timer);

    if should_add_return_trips {
        add_return_trips(&mut scenario, &mut rng);
    }
    if should_add_lunch_trips {
        add_lunch_trips(&mut scenario, &map, &mut timer);
    }

    scenario.save();
}

fn add_return_trips(scenario: &mut Scenario, rng: &mut XorShiftRng) {
    for person in &mut scenario.people {
        if person.trips.len() != 1 {
            continue;
        }

        // Assume a uniform distribution of 4-12 hour workday
        let depart =
            person.trips[0].depart + rand_duration(rng, Duration::hours(4), Duration::hours(12));
        person.trips.push(IndividTrip::new(
            depart,
            TripPurpose::Home,
            person.origin,
            person.trips[0].mode,
        ));
    }
}

fn add_lunch_trips(scenario: &mut Scenario, map: &Map, timer: &mut Timer) {}

fn rand_duration(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    Duration::seconds(rng.gen_range(low.inner_seconds()..high.inner_seconds()))
}

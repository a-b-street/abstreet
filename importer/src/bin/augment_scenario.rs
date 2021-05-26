//! A tool to modify each person's schedule from an existing scenario in various ways.
//!
//! `--add_return_trips`: For people with only a single trip, add a return trip back home sometime
//!                       4-12 hours later.
//! `--add_lunch_trips`: Before a person's final trip back home, insert a round-trip to a nearby
//!                      cafe or restaurant.
//!
//! These tools aren't very smart about detecting if a scenario already has these extra trips added
//! in; be careful about running this on the correct input. It modifies the given `--input` binary
//! scenario in-place.

#[macro_use]
extern crate log;

use rand::prelude::SliceRandom;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;

use abstutil::{prettyprint_usize, CmdArgs, Timer};
use geom::{Distance, Duration, FindClosest};
use map_model::{AmenityType, BuildingID, Map};
use sim::{IndividTrip, Scenario, TripEndpoint, TripMode, TripPurpose};

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
    let map = Map::load_synchronously(scenario.map_name.path(), &mut timer);

    if should_add_return_trips {
        add_return_trips(&mut scenario, &mut rng);
    }
    if should_add_lunch_trips {
        add_lunch_trips(&mut scenario, &map, &mut rng, &mut timer);
    }

    scenario.save();
}

fn add_return_trips(scenario: &mut Scenario, rng: &mut XorShiftRng) {
    let mut cnt = 0;
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
            person.trips[0].destination,
            person.trips[0].origin,
            person.trips[0].mode,
        ));
        cnt += 1;
    }
    info!("Added return trips to {} people", prettyprint_usize(cnt));
}

fn rand_duration(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    Duration::seconds(rng.gen_range(low.inner_seconds()..high.inner_seconds()))
}

fn add_lunch_trips(scenario: &mut Scenario, map: &Map, rng: &mut XorShiftRng, timer: &mut Timer) {
    // First let's build up a quadtree of lunch spots.
    timer.start("index lunch spots");
    let mut closest_spots: FindClosest<BuildingID> = FindClosest::new(map.get_bounds());
    for b in map.all_buildings() {
        if b.amenities
            .iter()
            .any(|a| AmenityType::categorize(&a.amenity_type) == Some(AmenityType::Food))
        {
            closest_spots.add(b.id, b.polygon.points());
        }
    }
    timer.stop("index lunch spots");

    timer.start_iter("add lunch trips", scenario.people.len());
    let mut cnt = 0;
    for person in &mut scenario.people {
        timer.next();
        let num_trips = person.trips.len();
        // Only handle people with their final trip going back home.
        if num_trips <= 1 || person.trips[num_trips - 1].destination != person.trips[0].origin {
            continue;
        }

        let work = match person.trips[num_trips - 2].destination {
            TripEndpoint::Bldg(b) => b,
            _ => continue,
        };
        let has_bike = person.trips[num_trips - 2].mode == TripMode::Bike;
        let (restaurant, mode) =
            if let Some(pair) = pick_lunch_spot(work, has_bike, &closest_spots, map, rng) {
                pair
            } else {
                continue;
            };

        // Insert the break in the middle of their workday
        let t1 = person.trips[num_trips - 2].depart;
        let t2 = person.trips[num_trips - 1].depart;
        let depart = t1 + (t2 - t1) / 2.0;
        let return_home = person.trips.pop().unwrap();
        person.trips.push(IndividTrip::new(
            depart,
            TripPurpose::Meal,
            TripEndpoint::Bldg(work),
            TripEndpoint::Bldg(restaurant),
            mode,
        ));
        person.trips.push(IndividTrip::new(
            depart + Duration::minutes(30),
            TripPurpose::Work,
            TripEndpoint::Bldg(restaurant),
            TripEndpoint::Bldg(work),
            mode,
        ));
        person.trips.push(return_home);
        cnt += 1;
    }
    info!("Added lunch trips to {} people", prettyprint_usize(cnt));
}

fn pick_lunch_spot(
    work: BuildingID,
    has_bike: bool,
    closest_spots: &FindClosest<BuildingID>,
    map: &Map,
    rng: &mut XorShiftRng,
) -> Option<(BuildingID, TripMode)> {
    // We have a list of candidate shops and the Euclidean distance there. Use that distance to
    // make a weighted choice.
    let choices =
        closest_spots.all_close_pts(map.get_b(work).polygon.center(), Distance::miles(10.0));
    let (b, _, dist) = choices
        .choose_weighted(rng, |(_, _, dist)| dist.inner_meters())
        .ok()?;
    // Simple hardcoded mode thresholds for now
    let mode = if *dist <= Distance::miles(1.0) {
        TripMode::Walk
    } else if has_bike {
        TripMode::Bike
    } else {
        TripMode::Drive
    };
    Some((*b, mode))
}

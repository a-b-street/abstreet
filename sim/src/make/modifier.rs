use crate::{IndividTrip, Scenario, SpawnTrip, TripMode};
use geom::Duration;
use map_model::Map;
use rand::Rng;
use rand_xorshift::XorShiftRng;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum ScenarioModifier {
    RepeatDays(usize),
    CancelPeople(usize),
    ChangeMode(usize, TripMode),
}

impl ScenarioModifier {
    // If this modifies scenario_name, then that means prebaked results don't match up and
    // shouldn't be used.
    pub fn apply(&self, map: &Map, s: Scenario, rng: &mut XorShiftRng) -> Scenario {
        match self {
            ScenarioModifier::RepeatDays(n) => repeat_days(s, *n),
            ScenarioModifier::CancelPeople(pct) => cancel_people(s, *pct, rng),
            ScenarioModifier::ChangeMode(pct, mode) => change_mode(map, s, *pct, *mode, rng),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            ScenarioModifier::RepeatDays(n) => format!("repeat the entire day {} times", n),
            ScenarioModifier::CancelPeople(pct) => {
                format!("cancel all trips for {}% of people", pct)
            }
            ScenarioModifier::ChangeMode(pct, mode) => {
                format!("change all trips for {}% of people to {}", pct, mode.verb())
            }
        }
    }
}

// Utter hack. Blindly repeats all trips taken by each person every day.
//
// What happens if the last place a person winds up in a day isn't the same as where their
// first trip the next starts? Will crash as soon as the scenario is instantiated, through
// check_schedule().
//
// The bigger problem is that any people that seem to require multiple cars... will wind up
// needing LOTS of cars.
fn repeat_days(mut s: Scenario, days: usize) -> Scenario {
    s.scenario_name = format!("{} (repeated {} days)", s.scenario_name, days);
    for person in &mut s.people {
        let mut trips = Vec::new();
        let mut offset = Duration::ZERO;
        for _ in 0..days {
            for trip in &person.trips {
                trips.push(IndividTrip {
                    depart: trip.depart + offset,
                    trip: trip.trip.clone(),
                    cancelled: false,
                });
            }
            offset += Duration::hours(24);
        }
        person.trips = trips;
    }
    s
}

fn cancel_people(mut s: Scenario, pct: usize, rng: &mut XorShiftRng) -> Scenario {
    let pct = (pct as f64) / 100.0;
    for person in &mut s.people {
        if rng.gen_bool(pct) {
            // TODO It's not obvious how to cancel individual trips. How are later trips affected?
            // What if a car doesn't get moved to another place?
            for trip in &mut person.trips {
                trip.cancelled = true;
            }
        }
    }
    s
}

fn change_mode(
    map: &Map,
    mut s: Scenario,
    pct: usize,
    mode: TripMode,
    rng: &mut XorShiftRng,
) -> Scenario {
    let pct = (pct as f64) / 100.0;
    for person in &mut s.people {
        if rng.gen_bool(pct) {
            for trip in &mut person.trips {
                if let Some(new) =
                    SpawnTrip::new(trip.trip.start(map), trip.trip.end(map), mode, map)
                {
                    trip.trip = new;
                }
            }
        }
    }
    s
}

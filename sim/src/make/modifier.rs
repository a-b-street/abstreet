use std::collections::BTreeSet;

use rand::Rng;
use rand_xorshift::XorShiftRng;
use serde::Deserialize;

use abstutil::Timer;
use geom::{Duration, Time};
use map_model::Map;

use crate::{IndividTrip, PersonID, Scenario, SpawnTrip, TripMode};

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Deserialize)]
pub enum ScenarioModifier {
    RepeatDays(usize),
    CancelPeople(usize),
    ChangeMode {
        to_mode: TripMode,
        pct_ppl: usize,
        departure_filter: (Time, Time),
        from_modes: BTreeSet<TripMode>,
    },
    // Scenario name
    AddExtraTrips(String),
}

impl ScenarioModifier {
    // If this modifies scenario_name, then that means prebaked results don't match up and
    // shouldn't be used.
    pub fn apply(&self, map: &Map, mut s: Scenario, rng: &mut XorShiftRng) -> Scenario {
        match self {
            ScenarioModifier::RepeatDays(n) => repeat_days(s, *n),
            ScenarioModifier::CancelPeople(pct) => cancel_people(s, *pct),
            ScenarioModifier::ChangeMode {
                to_mode,
                pct_ppl,
                departure_filter,
                from_modes,
            } => {
                let pct_ppl = (*pct_ppl as f64) / 100.0;
                for person in &mut s.people {
                    if !rng.gen_bool(pct_ppl) {
                        continue;
                    }
                    for trip in &mut person.trips {
                        if trip.depart < departure_filter.0 || trip.depart > departure_filter.1 {
                            continue;
                        }
                        if !from_modes.contains(&trip.trip.mode()) {
                            continue;
                        }
                        if let Some(new) =
                            SpawnTrip::new(trip.trip.start(map), trip.trip.end(map), *to_mode, map)
                        {
                            trip.modified = true;
                            trip.trip = new;
                        }
                    }
                }
                s
            }
            ScenarioModifier::AddExtraTrips(name) => {
                let other: Scenario = abstutil::read_binary(
                    abstutil::path_scenario(map.get_name(), name),
                    &mut Timer::throwaway(),
                );
                for mut p in other.people {
                    p.id = PersonID(s.people.len());
                    for trip in &mut p.trips {
                        trip.modified = true;
                    }
                    s.people.push(p);
                }
                s
            }
        }
    }

    pub fn describe(&self) -> String {
        match self {
            ScenarioModifier::RepeatDays(n) => format!("repeat the entire day {} times", n),
            ScenarioModifier::CancelPeople(pct) => {
                format!("cancel all trips for {}% of people", pct)
            }
            ScenarioModifier::ChangeMode {
                pct_ppl,
                to_mode,
                departure_filter,
                from_modes,
            } => format!(
                "change all trips for {}% of people of types {:?} leaving between {} and {} to {}",
                pct_ppl,
                from_modes,
                departure_filter.0.ampm_tostring(),
                departure_filter.1.ampm_tostring(),
                to_mode.verb()
            ),
            ScenarioModifier::AddExtraTrips(name) => format!("Add extra trips from {}", name),
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
                let mut new =
                    IndividTrip::new(trip.depart + offset, trip.purpose, trip.trip.clone());
                new.modified = true;
                trips.push(new);
            }
            offset += Duration::hours(24);
        }
        person.trips = trips;
    }
    s
}

// This is "stable" as percentage increases. If you cancel 10% of people in one run, then cancel 9%
// in another, the surviving people in the 9% run will be a strict superset of the 10% run.
fn cancel_people(mut s: Scenario, pct: usize) -> Scenario {
    for (idx, person) in s.people.iter_mut().enumerate() {
        if idx % 100 <= pct {
            // TODO It's not obvious how to cancel individual trips. How are later trips affected?
            // What if a car doesn't get moved to another place?
            for trip in &mut person.trips {
                trip.modified = true;
                trip.cancelled = true;
            }
        }
    }
    s
}

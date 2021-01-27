use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use abstutil::Timer;
use geom::{Duration, Time};
use map_model::Map;

use crate::{Scenario, TripMode};

/// Transforms an existing Scenario before instantiating it.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub enum ScenarioModifier {
    RepeatDays(usize),
    ChangeMode {
        pct_ppl: usize,
        departure_filter: (Time, Time),
        from_modes: BTreeSet<TripMode>,
        /// If `None`, then just cancel the trip.
        to_mode: Option<TripMode>,
    },
    /// Scenario name
    AddExtraTrips(String),
}

impl ScenarioModifier {
    /// If this modifies scenario_name, then that means prebaked results don't match up and
    /// shouldn't be used.
    pub fn apply(&self, map: &Map, mut s: Scenario) -> Scenario {
        match self {
            ScenarioModifier::RepeatDays(n) => repeat_days(s, *n),
            ScenarioModifier::ChangeMode {
                pct_ppl,
                departure_filter,
                from_modes,
                to_mode,
            } => {
                for (idx, person) in s.people.iter_mut().enumerate() {
                    // This is "stable" as percentage increases. If you modify 10% of people in one
                    // run, then modify 11% in another, the modified people in the 11% run will be
                    // a strict superset of the 10% run.
                    if idx % 100 > *pct_ppl {
                        continue;
                    }
                    let mut cancel_rest = false;
                    for trip in &mut person.trips {
                        if cancel_rest {
                            trip.modified = true;
                            trip.cancelled = true;
                            continue;
                        }

                        if trip.depart < departure_filter.0 || trip.depart > departure_filter.1 {
                            continue;
                        }
                        if !from_modes.contains(&trip.mode) {
                            continue;
                        }
                        if let Some(to_mode) = *to_mode {
                            trip.mode = to_mode;
                            trip.modified = true;
                        } else {
                            trip.modified = true;
                            trip.cancelled = true;
                            // The next trip assumes we're at the destination of this cancelled
                            // trip, and so on. Have to cancel the rest.
                            cancel_rest = true;
                        }
                    }
                }
                s
            }
            ScenarioModifier::AddExtraTrips(name) => {
                let other: Scenario = abstio::must_read_object(
                    abstio::path_scenario(map.get_name(), name),
                    &mut Timer::throwaway(),
                );
                for mut p in other.people {
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
            ScenarioModifier::ChangeMode {
                pct_ppl,
                to_mode,
                departure_filter,
                from_modes,
            } => format!(
                "change all trips for {}% of people of types {:?} leaving between {} and {} to \
                 {:?}",
                pct_ppl,
                from_modes,
                departure_filter.0.ampm_tostring(),
                departure_filter.1.ampm_tostring(),
                to_mode.map(|m| m.verb())
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
                let mut new = trip.clone();
                new.depart += offset;
                new.modified = true;
                trips.push(new);
            }
            offset += Duration::hours(24);
        }
        person.trips = trips;
    }
    s
}

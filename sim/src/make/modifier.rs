use crate::{IndividTrip, Scenario};
use geom::Duration;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum ScenarioModifier {
    RepeatDays(usize),
}

impl ScenarioModifier {
    pub fn apply(&self, s: Scenario) -> Scenario {
        let mut s = match self {
            ScenarioModifier::RepeatDays(n) => repeat_days(s, *n),
        };
        s.scenario_name = format!("{} (modified)", s.scenario_name);
        s
    }

    pub fn describe(&self) -> String {
        match self {
            ScenarioModifier::RepeatDays(n) => format!("repeat the entire day {} times", n),
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
    for person in &mut s.people {
        let mut trips = Vec::new();
        let mut offset = Duration::ZERO;
        for _ in 0..days {
            for trip in &person.trips {
                trips.push(IndividTrip {
                    depart: trip.depart + offset,
                    trip: trip.trip.clone(),
                });
            }
            offset += Duration::hours(24);
        }
        person.trips = trips;
    }
    s
}

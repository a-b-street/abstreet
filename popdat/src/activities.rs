use rand::Rng;
use rand_xorshift::XorShiftRng;

use geom::{Duration, Time};

use crate::{Activity, CensusPerson, Config, PersonType, Schedule};

impl CensusPerson {
    pub fn generate_schedule(&self, _config: &Config, rng: &mut XorShiftRng) -> Schedule {
        // TODO How do we pick these categories based on census data?
        let person_type = if rng.gen_bool(0.5) {
            PersonType::Student
        } else {
            PersonType::Worker
        };

        // Fill out a list of activities and how long the person should do the activity before
        // travelling to the next place.
        let mut plan = Vec::new();
        let start_time;

        match person_type {
            PersonType::Student => {
                // I'm probably channeling a college student here...
                start_time = rand_time(rng, hours(8), hours(11));
                if rng.gen_bool(0.95) {
                    plan.push((Activity::Breakfast, minutes(30)));
                }
                plan.push((Activity::School, rand_duration(rng, hours(3), hours(6))));
                if rng.gen_bool(0.3) {
                    plan.push((
                        Activity::Lunch,
                        rand_duration(rng, minutes(20), minutes(40)),
                    ));
                }
                plan.push((Activity::School, rand_duration(rng, hours(2), hours(4))));
                if rng.gen_bool(0.6) {
                    plan.push((Activity::Entertainment, hours(2)));
                } else {
                    plan.push((Activity::Errands, rand_duration(rng, minutes(15), hours(1))));
                }
                // The last duration doesn't matter
                plan.push((Activity::Home, hours(8)));
            }
            PersonType::Worker => {
                start_time = rand_time(rng, hours(6), hours(9));
                if rng.gen_bool(0.8) {
                    plan.push((Activity::Breakfast, minutes(15)));
                }
                plan.push((Activity::Work, rand_duration(rng, hours(4), hours(5))));
                plan.push((
                    Activity::Lunch,
                    rand_duration(rng, minutes(20), minutes(40)),
                ));
                plan.push((Activity::Work, hours(4)));
                if rng.gen_bool(0.8) {
                    plan.push((Activity::Errands, rand_duration(rng, minutes(15), hours(1))));
                }
                // The last duration doesn't matter
                plan.push((Activity::Home, hours(8)));
            }
        }

        let mut schedule = Vec::new();
        let mut now = start_time;
        for (activity, duration) in plan {
            schedule.push((now, activity));
            // TODO We have to add in commute time here, but at this stage in the pipeline, we have
            // no idea...
            now += rand_duration(rng, Duration::minutes(30), Duration::hours(1));
            now += duration;
        }
        Schedule {
            activities: schedule,
        }
    }
}

fn rand_duration(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Duration {
    assert!(high > low);
    Duration::seconds(rng.gen_range(low.inner_seconds()..high.inner_seconds()))
}

fn rand_time(rng: &mut XorShiftRng, low: Duration, high: Duration) -> Time {
    Time::START_OF_DAY + rand_duration(rng, low, high)
}

// TODO I thought we could just use geom::Duration::{hours, minutes};   but this doesn't work
fn minutes(x: usize) -> Duration {
    Duration::minutes(x)
}
fn hours(x: usize) -> Duration {
    Duration::hours(x)
}

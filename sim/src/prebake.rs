use serde::Serialize;

use crate::{AlertHandler, Sim, SimFlags, SimOptions};
use abstutil::{prettyprint_usize, Timer};
use geom::{Duration, Time};
use map_model::Map;
use synthpop::Scenario;

/// Simulate a curated list of scenarios to completion, and save the analytics as "prebaked
/// results," to later compare simulation metrics against the baseline without map edits.
pub fn prebake(map: &Map, scenario: Scenario, timer: &mut Timer) -> PrebakeSummary {
    timer.start(format!(
        "prebake for {} / {}",
        scenario.map_name.describe(),
        scenario.scenario_name
    ));

    let mut opts = SimOptions::new("prebaked");
    opts.alerts = AlertHandler::Silence;
    let mut sim = Sim::new(map, opts);
    // Bit of an abuse of this, but just need to fix the rng seed.
    let mut rng = SimFlags::for_test("prebaked").make_rng();
    sim.instantiate(&scenario, map, &mut rng, timer);

    // Run until a few hours after the end of the day. Some trips start close to midnight, and we
    // want prebaked data for them too.
    sim.timed_step(
        map,
        sim.get_end_of_day() - Time::START_OF_DAY + Duration::hours(3),
        &mut None,
        timer,
    );
    abstio::write_binary(
        abstio::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
        sim.get_analytics(),
    );
    // TODO Remove the num_agents check once transit isn't as broken. In sao_miguel_paulista,
    // people wait for a bus that stops running at midnight.
    if !sim.is_done() && sim.num_agents().sum() > 200 {
        panic!(
            "It's {} and there are still {} agents left in {}. Gridlock likely...",
            sim.time(),
            prettyprint_usize(sim.num_agents().sum()),
            scenario.map_name.describe()
        );
    }
    timer.stop(format!(
        "prebake for {} / {}",
        scenario.map_name.describe(),
        scenario.scenario_name
    ));

    PrebakeSummary::new(&sim, &scenario)
}

#[derive(Debug, Serialize)]
pub struct PrebakeSummary {
    pub map: String,
    pub scenario: String,
    pub finished_trips: usize,
    pub cancelled_trips: usize,
    pub total_trip_duration_seconds: f64,
}

impl PrebakeSummary {
    pub fn new(sim: &Sim, scenario: &Scenario) -> Self {
        let mut finished_trips = 0;
        let mut cancelled_trips = 0;
        // Use f64 seconds, since a serialized Duration has a low cap.
        let mut total_trip_duration_seconds = 0.0;
        for (_, _, _, maybe_duration) in &sim.get_analytics().finished_trips {
            if let Some(dt) = maybe_duration {
                finished_trips += 1;
                total_trip_duration_seconds += dt.inner_seconds();
            } else {
                cancelled_trips += 1;
            }
        }
        Self {
            map: scenario.map_name.describe(),
            scenario: scenario.scenario_name.clone(),
            finished_trips,
            cancelled_trips,
            total_trip_duration_seconds,
        }
    }
}

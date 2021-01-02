use abstio::MapName;
use abstutil::{prettyprint_usize, Timer};
use geom::{Duration, Time};
use map_model::Map;
use sim::{AlertHandler, Scenario, Sim, SimFlags, SimOptions};

use crate::sandbox::TutorialState;

/// Simulate a curated list of scenarios to completion, and save the analytics as "prebaked
/// results," to later compare simulation metrics against the baseline without map edits.
pub fn prebake_all() {
    let mut timer = Timer::new("prebake all challenge results");

    {
        let map = map_model::Map::new(MapName::seattle("montlake").path(), &mut timer);
        let scenario: Scenario =
            abstio::read_binary(abstio::path_scenario(map.get_name(), "weekday"), &mut timer);
        prebake(&map, scenario, None, &mut timer);

        for generator in TutorialState::scenarios_to_prebake(&map) {
            let scenario = generator.generate(
                &map,
                &mut SimFlags::for_test("prebaked").make_rng(),
                &mut timer,
            );
            prebake(&map, scenario, None, &mut timer);
        }
    }

    for name in vec![MapName::seattle("lakeslice")] {
        let map = map_model::Map::new(name.path(), &mut timer);
        let scenario: Scenario =
            abstio::read_binary(abstio::path_scenario(map.get_name(), "weekday"), &mut timer);
        prebake(&map, scenario, None, &mut timer);
    }
}

fn prebake(map: &Map, scenario: Scenario, time_limit: Option<Duration>, timer: &mut Timer) {
    timer.start(format!(
        "prebake for {} / {}",
        scenario.map_name.describe(),
        scenario.scenario_name
    ));

    let mut opts = SimOptions::new("prebaked");
    opts.alerts = AlertHandler::Silence;
    let mut sim = Sim::new(&map, opts, timer);
    // Bit of an abuse of this, but just need to fix the rng seed.
    let mut rng = SimFlags::for_test("prebaked").make_rng();
    scenario.instantiate(&mut sim, &map, &mut rng, timer);
    if let Some(dt) = time_limit {
        sim.timed_step(&map, dt, &mut None, timer);
    } else {
        sim.timed_step(
            &map,
            sim.get_end_of_day() - Time::START_OF_DAY,
            &mut None,
            timer,
        );
    }

    abstio::write_binary(
        abstio::path_prebaked_results(&scenario.map_name, &scenario.scenario_name),
        sim.get_analytics(),
    );
    let agents_left = sim.num_agents().sum();
    timer.note(format!("{} agents left by end of day", agents_left));
    timer.stop(format!(
        "prebake for {} / {}",
        scenario.map_name.describe(),
        scenario.scenario_name
    ));

    if agents_left > 500 {
        panic!(
            "{} agents left by end of day on {}; gridlock may be likely",
            prettyprint_usize(agents_left),
            scenario.map_name.describe()
        );
    }
}

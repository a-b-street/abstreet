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
        let map =
            map_model::Map::load_synchronously(MapName::seattle("montlake").path(), &mut timer);
        for generator in TutorialState::scenarios_to_prebake(&map) {
            let scenario = generator.generate(
                &map,
                &mut SimFlags::for_test("prebaked").make_rng(),
                &mut timer,
            );
            prebake(&map, scenario, None, &mut timer);
        }
    }

    for name in vec![
        MapName::seattle("arboretum"),
        MapName::seattle("greenlake"),
        MapName::seattle("montlake"),
        MapName::seattle("lakeslice"),
        //MapName::seattle("phinney"),
        MapName::seattle("qa"),
        MapName::seattle("rainier_valley"),
        //MapName::seattle("wallingford"),
    ] {
        let map = map_model::Map::load_synchronously(name.path(), &mut timer);
        let scenario: Scenario =
            abstio::read_binary(abstio::path_scenario(map.get_name(), "weekday"), &mut timer);
        prebake(&map, scenario, None, &mut timer);
    }

    for scenario_name in ["base", "go_active", "base_with_bg", "go_active_with_bg"] {
        let map = map_model::Map::load_synchronously(
            MapName::new("gb", "poundbury", "center").path(),
            &mut timer,
        );
        let scenario: Scenario = abstio::read_binary(
            abstio::path_scenario(map.get_name(), scenario_name),
            &mut timer,
        );
        let mut opts = SimOptions::new("prebaked");
        opts.alerts = AlertHandler::Silence;
        opts.infinite_parking = true;
        prebake(&map, scenario, Some(opts), &mut timer);
    }
}

fn prebake(map: &Map, scenario: Scenario, opts: Option<SimOptions>, timer: &mut Timer) {
    timer.start(format!(
        "prebake for {} / {}",
        scenario.map_name.describe(),
        scenario.scenario_name
    ));

    let opts = opts.unwrap_or_else(|| {
        let mut opts = SimOptions::new("prebaked");
        opts.alerts = AlertHandler::Silence;
        opts
    });
    let mut sim = Sim::new(map, opts);
    // Bit of an abuse of this, but just need to fix the rng seed.
    let mut rng = SimFlags::for_test("prebaked").make_rng();
    scenario.instantiate(&mut sim, map, &mut rng, timer);

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
    // TODO Remove the num_agents check once transit isn't broken. In Green Lake, 3 poor people are
    // waiting at a bus stop that'll never be served...
    if !sim.is_done() && sim.num_agents().sum() > 10 {
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
}

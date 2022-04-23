use abstio::MapName;
use abstutil::Timer;
use sim::prebake::prebake;
use sim::{ScenarioGenerator, SimFlags};
use synthpop::Scenario;

use crate::sandbox::TutorialState;

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
            // Don't record a summary for this
            prebake(&map, scenario, &mut timer);
        }
    }

    let mut summaries = Vec::new();
    for name in vec![
        MapName::seattle("arboretum"),
        MapName::seattle("montlake"),
        //MapName::seattle("lakeslice"),
        //MapName::seattle("phinney"),
        //MapName::seattle("qa"),
        //MapName::seattle("wallingford"),
    ] {
        let map = map_model::Map::load_synchronously(name.path(), &mut timer);
        let scenario: Scenario =
            abstio::read_binary(abstio::path_scenario(map.get_name(), "weekday"), &mut timer);
        summaries.push(prebake(&map, scenario, &mut timer));
    }

    // Since adding off-map traffic, these all gridlock now
    if false {
        let pbury_map = map_model::Map::load_synchronously(
            MapName::new("gb", "poundbury", "center").path(),
            &mut timer,
        );
        for scenario_name in ["base", "go_active", "base_with_bg", "go_active_with_bg"] {
            let scenario: Scenario = abstio::read_binary(
                abstio::path_scenario(pbury_map.get_name(), scenario_name),
                &mut timer,
            );
            summaries.push(prebake(&pbury_map, scenario, &mut timer));
        }
    }

    {
        let tehran_map = map_model::Map::load_synchronously(
            MapName::new("ir", "tehran", "parliament").path(),
            &mut timer,
        );
        let scenario = ScenarioGenerator::proletariat_robot(
            &tehran_map,
            &mut SimFlags::for_test("prebaked").make_rng(),
            &mut timer,
        );
        summaries.push(prebake(&tehran_map, scenario, &mut timer));
    }

    {
        let map = map_model::Map::load_synchronously(
            MapName::new("br", "sao_paulo", "sao_miguel_paulista").path(),
            &mut timer,
        );
        let scenario: Scenario =
            abstio::read_binary(abstio::path_scenario(map.get_name(), "Full"), &mut timer);
        summaries.push(prebake(&map, scenario, &mut timer));
    }

    // Assume this is being run from the root directory (via import.sh). This other tests directory
    // is the most appropriate place to keep this.
    abstio::write_json(
        "tests/goldenfiles/prebaked_summaries.json".to_string(),
        &summaries,
    );
}

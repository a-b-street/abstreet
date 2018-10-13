pub mod a_b_tests;
pub mod chokepoints;
pub mod classification;
pub mod color_picker;
pub mod debug_objects;
pub mod draw_neighborhoods;
pub mod floodfill;
pub mod follow;
pub mod geom_validation;
pub mod hider;
pub mod layers;
pub mod logs;
pub mod map_edits;
pub mod road_editor;
pub mod scenarios;
pub mod search;
pub mod show_route;
pub mod sim_controls;
pub mod steep;
pub mod stop_sign_editor;
pub mod traffic_signal_editor;
pub mod turn_cycler;
pub mod warp;

use abstutil;
use ezgui::{Color, WrappedWizard};
use map_model::Map;
use objects::{Ctx, ID};
use sim::{ABTest, Neighborhood, Scenario, Tick};

pub trait Colorizer {
    fn color_for(&self, _obj: ID, _ctx: Ctx) -> Option<Color> {
        None
    }
}

// TODO Further refactoring should be done, but at least group these here to start.
// General principles are to avoid actually deserializing the objects unless needed.

pub fn choose_neighborhood(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    // Load the full object, since various plugins visualize the neighborhood when menuing over it
    wizard
        .choose_something::<Neighborhood>(
            query,
            Box::new(move || abstutil::load_all_objects("neighborhoods", &map_name)),
        ).map(|(n, _)| n)
}

pub fn load_neighborhood(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<Neighborhood> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<Neighborhood>(
            query,
            Box::new(move || abstutil::load_all_objects("neighborhoods", &map_name)),
        ).map(|(_, n)| n)
}

pub fn load_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<Scenario> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<Scenario>(
            query,
            Box::new(move || abstutil::load_all_objects("scenarios", &map_name)),
        ).map(|(_, s)| s)
}

pub fn choose_scenario(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<String>(
            query,
            Box::new(move || abstutil::list_all_objects("scenarios", &map_name)),
        ).map(|(n, _)| n)
}

pub fn choose_edits(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<String> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<String>(
            query,
            Box::new(move || abstutil::list_all_objects("edits", &map_name)),
        ).map(|(n, _)| n)
}

pub fn load_ab_test(map: &Map, wizard: &mut WrappedWizard, query: &str) -> Option<ABTest> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something::<ABTest>(
            query,
            Box::new(move || abstutil::load_all_objects("ab_tests", &map_name)),
        ).map(|(_, t)| t)
}

pub fn input_tick(wizard: &mut WrappedWizard, query: &str) -> Option<Tick> {
    wizard.input_something(query, Box::new(|line| Tick::parse(&line)))
}

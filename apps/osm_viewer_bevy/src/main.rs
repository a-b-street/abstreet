use abstutil;
use bevy::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use colors::ColorScheme;
use map_model::Map;
use map_renderer::road::RoadBundle;

mod colors;
mod map_renderer;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    Loading,
    Running,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(PanCamPlugin::default())
        .add_startup_system(setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let mut timer = abstutil::time::Timer::new("load_map");
    let map_model = Map::load_synchronously(
        "../../data/system/us/seattle/maps/montlake.bin".to_string(),
        &mut timer,
    );

    let color_scheme = ColorScheme::new(colors::ColorSchemeChoice::ClassicDayMode);

    for road in map_model.all_roads().iter() {
        commands.spawn(RoadBundle::new(
            road,
            &mut meshes,
            &mut materials,
            &color_scheme,
        ));
    }

    commands.spawn((Camera2dBundle::default(), PanCam::default()));
}

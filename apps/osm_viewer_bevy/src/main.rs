use abstutil;
use bevy::prelude::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use colors::ColorScheme;
use map_model::Map;
use map_renderer::{intersection::IntersectionBundle, road::RoadBundle};

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

    for intersection in map_model.all_intersections().iter() {
        commands.spawn(IntersectionBundle::new(
            intersection,
            &map_model,
            &mut meshes,
            &mut materials,
            &color_scheme,
        ));
    }

    let map_bounds = map_model.get_bounds();

    let camera_bundle = Camera2dBundle {
        transform: Transform::from_xyz(
            map_bounds.max_x as f32 / 2.,
            -map_bounds.max_y as f32 / 2.,
            0.,
        ),
        ..default()
    };

    commands.spawn((camera_bundle, PanCam::default()));
}

use abstutil;
use bevy::{prelude::*, sprite::MaterialMesh2dBundle};
use bevy_mod_picking::{DefaultPickingPlugins, PickingCameraBundle};
use bevy_pancam::{PanCam, PanCamPlugin};
use colors::ColorScheme;
use map_model::Map;
use map_renderer::{
    area::AreaBundle,
    details_layer::{toggle_details_visibility, DetailsLayerBundle},
    intersection::IntersectionBundle,
    lane::LaneBundle,
    map_layer::MapLayerBundle,
    road::RoadBundle,
};

mod colors;
mod map_renderer;
mod mesh_builder;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum AppState {
    Loading,
    Running,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(PanCamPlugin::default())
        .add_plugins(DefaultPickingPlugins)
        .add_startup_system(setup)
        .add_system(toggle_details_visibility)
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

    let map_bounds = map_model.get_bounds();

    let color_scheme = ColorScheme::new(colors::ColorSchemeChoice::DayMode);
    commands
        .spawn(MapLayerBundle::default())
        .with_children(|map_layer| {
            map_layer.spawn(MaterialMesh2dBundle {
                mesh: meshes
                    .add(Mesh::from(shape::Quad::flipped(Vec2::new(
                        map_bounds.width() as f32,
                        map_bounds.height() as f32,
                    ))))
                    .into(),

                material: materials.add(ColorMaterial::from(color_scheme.map_background)),
                ..default()
            });

            for area in map_model.all_areas().iter() {
                map_layer.spawn(AreaBundle::new(
                    area,
                    &mut meshes,
                    &mut materials,
                    &color_scheme,
                ));
            }

            for road in map_model.all_roads().iter() {
                map_layer.spawn(RoadBundle::new(
                    road,
                    &mut meshes,
                    &mut materials,
                    &color_scheme,
                ));
            }

            for intersection in map_model.all_intersections().iter() {
                map_layer.spawn(IntersectionBundle::new(
                    intersection,
                    &map_model,
                    &mut meshes,
                    &mut materials,
                    &color_scheme,
                ));
            }
        });

    let camera_bundle = Camera2dBundle {
        transform: Transform::from_xyz(
            map_bounds.max_x as f32 / 2.,
            -map_bounds.max_y as f32 / 2.,
            0.,
        ),
        ..default()
    };

    commands
        .spawn(DetailsLayerBundle::default())
        .with_children(|details_layer| {
            for road in map_model.all_roads().iter() {
                if !road.is_light_rail() {
                    for lane in road.lanes.iter() {
                        details_layer.spawn(LaneBundle::new(
                            lane,
                            road,
                            &mut meshes,
                            &mut materials,
                            &color_scheme,
                        ));
                    }
                }
            }
        });

    commands.spawn((
        camera_bundle,
        PickingCameraBundle::default(),
        PanCam::default(),
    ));
}

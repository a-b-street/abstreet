use abstutil;
use bevy::{prelude::*, sprite::MaterialMesh2dBundle};
use bevy_earcutr::*;
use bevy_pancam::{PanCam, PanCamPlugin};
use map_model::Map;

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

    for road in map_model.all_roads().iter() {
        let poly = road.get_thick_polygon();

        let mut builder = PolygonMeshBuilder::new();
        // Call `add_earcutr_input` or each polygon you want in the mesh.
        builder.add_earcutr_input(EarcutrInput {
            vertices: poly
                .get_outer_ring()
                .points()
                .iter()
                .flat_map(|p| vec![p.x(), p.y()])
                .collect::<Vec<f64>>(),
            interior_indices: vec![],
        });

        if let Some(mesh) = builder.build() {
            commands.spawn(MaterialMesh2dBundle {
                mesh: meshes.add(Mesh::from(mesh)).into(),
                material: materials.add(ColorMaterial::from(Color::PURPLE)),
                ..default()
            });
        };
    }

    commands.spawn((Camera2dBundle::default(), PanCam::default()));
}

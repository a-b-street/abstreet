use bevy::{
    prelude::*,
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};

use map_model::{Road, RoadID};

use crate::{colors::ColorScheme, mesh_builder::build_mesh_from_polygon};

#[derive(Component)]
pub struct RoadIdComponent(pub RoadID);

#[derive(Bundle)]
pub struct RoadBundle {
    road_id: RoadIdComponent,

    #[bundle]
    mesh: MaterialMesh2dBundle<ColorMaterial>,
}

impl RoadBundle {
    pub fn new(
        road: &Road,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<ColorMaterial>>,
        color_scheme: &ColorScheme,
    ) -> Self {
        let mesh = build_mesh_from_polygon(road.get_thick_polygon());

        RoadBundle {
            road_id: RoadIdComponent(road.id),
            mesh: MaterialMesh2dBundle {
                mesh: meshes.add(mesh).into(),
                transform: Transform::from_xyz(0., 0., 10.0 + road.zorder as f32),
                material: materials.add(ColorMaterial::from(
                    color_scheme.unzoomed_road_surface(road.get_rank()),
                )),
                ..default()
            },
        }
    }
}

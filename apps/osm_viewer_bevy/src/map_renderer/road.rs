use std::f32::consts::PI;

use bevy::{
    prelude::*,
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};
use bevy_earcutr::{build_mesh_from_earcutr, EarcutrResult};
use geom::Tessellation;
use map_model::{Map, Road};

use crate::colors::ColorScheme;

#[derive(Component)]
struct RoadComponent(Road);

#[derive(Bundle)]
pub struct RoadBundle {
    road: RoadComponent,

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
        let polygon = road.get_thick_polygon();
        let earcutr_output = Tessellation::from(polygon).consume();

        let mesh = build_mesh_from_earcutr(
            EarcutrResult {
                vertices: earcutr_output
                    .0
                    .iter()
                    .flat_map(|p| vec![p.x(), p.y()])
                    .collect::<Vec<f64>>(),
                triangle_indices: earcutr_output
                    .1
                    .iter()
                    .rev()
                    .map(|i| *i as usize)
                    .collect::<Vec<usize>>(),
            },
            0.,
        );

        RoadBundle {
            road: RoadComponent(road.to_owned()),
            mesh: MaterialMesh2dBundle {
                transform: Transform::from_rotation(Quat::from_rotation_x(PI)),
                mesh: meshes.add(mesh).into(),
                material: materials.add(ColorMaterial::from(
                    color_scheme.unzoomed_road_surface(road.get_rank()),
                )),
                ..default()
            },
        }
    }
}

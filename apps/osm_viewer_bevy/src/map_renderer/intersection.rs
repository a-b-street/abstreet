use std::f32::consts::PI;

use bevy::{
    prelude::*,
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};
use bevy_earcutr::{build_mesh_from_earcutr, EarcutrResult};
use geom::Tessellation;
use map_model::{Intersection, Map};

use crate::colors::ColorScheme;

#[derive(Component)]
struct IntersectionComponent(Intersection);

#[derive(Bundle)]
pub struct IntersectionBundle {
    intersection: IntersectionComponent,

    #[bundle]
    mesh: MaterialMesh2dBundle<ColorMaterial>,
}

impl IntersectionBundle {
    pub fn new(
        intersection: &Intersection,
        map: &Map,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<ColorMaterial>>,
        color_scheme: &ColorScheme,
    ) -> Self {
        let earcutr_output = Tessellation::from(intersection.polygon.to_owned()).consume();

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

        IntersectionBundle {
            intersection: IntersectionComponent(intersection.to_owned()),

            mesh: MaterialMesh2dBundle {
                transform: Transform::from_rotation(Quat::from_rotation_x(PI)),
                mesh: meshes.add(mesh).into(),
                material: materials.add(ColorMaterial::from(
                    color_scheme.unzoomed_road_surface(intersection.get_rank(map)),
                )),
                ..default()
            },
        }
    }
}

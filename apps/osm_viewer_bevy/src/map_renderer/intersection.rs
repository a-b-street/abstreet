use bevy::{
    prelude::*,
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};

use map_model::{Intersection, Map};

use crate::{colors::ColorScheme, mesh_builder::build_mesh_from_polygon};

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
        let mesh = build_mesh_from_polygon(intersection.polygon.to_owned());

        IntersectionBundle {
            intersection: IntersectionComponent(intersection.to_owned()),

            mesh: MaterialMesh2dBundle {
                mesh: meshes.add(mesh).into(),
                transform: Transform::from_xyz(0., 0., 10.0 + intersection.get_zorder(map) as f32),
                material: materials.add(ColorMaterial::from(
                    color_scheme.unzoomed_road_surface(intersection.get_rank(map)),
                )),
                ..default()
            },
        }
    }
}

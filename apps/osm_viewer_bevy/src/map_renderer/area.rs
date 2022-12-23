use bevy::{
    prelude::*,
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};
use bevy_earcutr::{build_mesh_from_earcutr, EarcutrResult};
use geom::Tessellation;
use map_model::{Area, AreaType};

use crate::colors::ColorScheme;

#[derive(Component)]
struct AreaComponent(Area);

#[derive(Bundle)]
pub struct AreaBundle {
    area: AreaComponent,

    #[bundle]
    mesh: MaterialMesh2dBundle<ColorMaterial>,
}

impl AreaBundle {
    pub fn new(
        area: &Area,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<ColorMaterial>>,
        color_scheme: &ColorScheme,
    ) -> Self {
        let earcutr_output = Tessellation::from(area.polygon.clone()).consume();

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

        AreaBundle {
            area: AreaComponent(area.to_owned()),
            mesh: MaterialMesh2dBundle {
                mesh: meshes.add(mesh).into(),
                transform: Transform::from_xyz(0., 0., 200.0),
                material: materials.add(ColorMaterial::from(match area.area_type {
                    AreaType::Park => color_scheme.grass,
                    AreaType::Water => color_scheme.water,
                    AreaType::Island => color_scheme.map_background,
                    AreaType::StudyArea => color_scheme.study_area,
                })),
                ..default()
            },
        }
    }
}

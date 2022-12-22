use bevy::{
    prelude::*,
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};
use bevy_earcutr::{build_mesh_from_earcutr, EarcutrResult};
use bevy_mod_picking::PickableBundle;
use geom::Tessellation;
use map_model::{Lane, Road};

use crate::colors::ColorScheme;

#[derive(Component)]
struct LaneComponent(Lane);

#[derive(Bundle)]
pub struct LaneBundle {
    lane: LaneComponent,

    #[bundle]
    mesh: (MaterialMesh2dBundle<ColorMaterial>, PickableBundle),
}

impl LaneBundle {
    pub fn new(
        lane: &Lane,
        road: &Road,
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<ColorMaterial>>,
        color_scheme: &ColorScheme,
    ) -> Self {
        let polygon = lane.get_thick_polygon();
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

        LaneBundle {
            lane: LaneComponent(lane.to_owned()),
            mesh: (
                MaterialMesh2dBundle {
                    mesh: meshes.add(mesh).into(),
                    material: materials.add(ColorMaterial::from(
                        color_scheme.zoomed_road_surface(lane.lane_type, road.get_rank()),
                    )),
                    ..default()
                },
                PickableBundle::default(),
            ),
        }
    }
}

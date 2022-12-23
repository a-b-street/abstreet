use bevy::{
    prelude::*,
    sprite::{ColorMaterial, MaterialMesh2dBundle},
};
use bevy_mod_picking::PickableBundle;
use map_model::{Lane, LaneID, Road};

use crate::{colors::ColorScheme, mesh_builder::build_mesh_from_polygon};

#[derive(Component)]
pub struct LaneIdComponent(pub LaneID);

#[derive(Bundle)]
pub struct LaneBundle {
    lane: LaneIdComponent,

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
        let mesh = build_mesh_from_polygon(lane.get_thick_polygon());

        LaneBundle {
            lane: LaneIdComponent(lane.id),
            mesh: (
                MaterialMesh2dBundle {
                    mesh: meshes.add(mesh).into(),
                    transform: Transform::from_xyz(0., 0., 100.0 + road.zorder as f32),
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

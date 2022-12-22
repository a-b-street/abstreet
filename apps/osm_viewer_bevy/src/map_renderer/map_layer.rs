use std::f32::consts::PI;

use bevy::prelude::*;

#[derive(Bundle)]
pub struct MapLayerBundle {
    spatial_bundle: SpatialBundle,
}

impl Default for MapLayerBundle {
    fn default() -> Self {
        MapLayerBundle {
            spatial_bundle: SpatialBundle {
                transform: Transform::from_rotation(Quat::from_rotation_x(PI))
                    .with_translation(Vec3::new(0., 0., -2.)),
                ..default()
            },
        }
    }
}

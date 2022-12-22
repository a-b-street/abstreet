use std::f32::consts::PI;

use bevy::prelude::*;

#[derive(Bundle)]
pub struct DetailsLayerBundle {
    spatial_bundle: SpatialBundle,
}

impl Default for DetailsLayerBundle {
    fn default() -> Self {
        DetailsLayerBundle {
            spatial_bundle: SpatialBundle {
                transform: Transform::from_rotation(Quat::from_rotation_x(PI))
                    .with_translation(Vec3::new(0., 0., -1.)),
                ..default()
            },
        }
    }
}

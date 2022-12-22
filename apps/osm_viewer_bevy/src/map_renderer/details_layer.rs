use std::f32::consts::PI;

use bevy::prelude::*;

#[derive(Component)]
pub struct DetailsLayer;

#[derive(Bundle)]
pub struct DetailsLayerBundle {
    details_layer: DetailsLayer,
    spatial_bundle: SpatialBundle,
}

impl Default for DetailsLayerBundle {
    fn default() -> Self {
        DetailsLayerBundle {
            details_layer: DetailsLayer,
            spatial_bundle: SpatialBundle {
                transform: Transform::from_rotation(Quat::from_rotation_x(PI))
                    .with_translation(Vec3::new(0., 0., -1.)),
                ..default()
            },
        }
    }
}

pub fn toggle_details_visibility(
    camera_projection: Query<&OrthographicProjection, With<Camera2d>>,
    mut details_visibility: Query<&mut Visibility, With<DetailsLayer>>,
) {
    details_visibility.single_mut().is_visible = camera_projection.single().scale < 0.75;
}

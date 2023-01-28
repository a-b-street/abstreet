use bevy::prelude::*;

#[derive(Bundle)]
pub struct MapLayerBundle {
    spatial_bundle: SpatialBundle,
}

impl Default for MapLayerBundle {
    fn default() -> Self {
        MapLayerBundle {
            spatial_bundle: SpatialBundle::default(),
        }
    }
}

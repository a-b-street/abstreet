use abstutil;
use bevy::{prelude::*, sprite::MaterialMesh2dBundle};
use bevy_egui::EguiPlugin;
use bevy_mod_picking::{DefaultPickingPlugins, PickingCameraBundle, PickingEvent, SelectionEvent};
use bevy_pancam::{PanCam, PanCamPlugin};
use colors::ColorScheme;
use map_model::{LaneID, Map};
use map_renderer::{
    area::AreaBundle,
    details_layer::{toggle_details_visibility, DetailsLayerBundle},
    intersection::IntersectionBundle,
    lane::{LaneBundle, LaneIdComponent},
    map_layer::MapLayerBundle,
    road::RoadBundle,
};
use mesh_builder::build_mesh_from_polygon;

mod colors;
mod map_renderer;
mod mesh_builder;

#[derive(Default, Resource)]
struct UiState {
    selected_lane_id: Option<LaneID>,
}

#[derive(Resource)]
struct MapResource {
    map_model: Map,
}

impl FromWorld for MapResource {
    fn from_world(_world: &mut World) -> Self {
        let mut timer = abstutil::time::Timer::new("load_map");

        let map_model = Map::load_synchronously(
            "../../data/system/us/seattle/maps/montlake.bin".to_string(),
            &mut timer,
        );
        MapResource { map_model }
    }
}

fn main() {
    App::new()
        .init_resource::<UiState>()
        .init_resource::<MapResource>()
        .add_plugins(DefaultPlugins)
        .add_plugin(PanCamPlugin::default())
        .add_plugins(DefaultPickingPlugins)
        .add_plugin(EguiPlugin)
        .add_startup_system(setup)
        .add_system(toggle_details_visibility)
        .add_system_to_stage(CoreStage::PostUpdate, handle_selection)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    map_resource: Res<MapResource>,
) {
    let map_model = &map_resource.map_model;
    let map_bounds = map_model.get_bounds();

    let color_scheme = ColorScheme::new(colors::ColorSchemeChoice::DayMode);
    commands
        .spawn(MapLayerBundle::default())
        .with_children(|map_layer| {
            map_layer.spawn(MaterialMesh2dBundle {
                mesh: meshes
                    .add(build_mesh_from_polygon(
                        map_model.get_boundary_polygon().to_owned(),
                    ))
                    .into(),

                material: materials.add(ColorMaterial::from(color_scheme.map_background)),
                ..default()
            });

            for area in map_model.all_areas().iter() {
                map_layer.spawn(AreaBundle::new(
                    area,
                    &mut meshes,
                    &mut materials,
                    &color_scheme,
                ));
            }

            for road in map_model.all_roads().iter() {
                map_layer.spawn(RoadBundle::new(
                    road,
                    &mut meshes,
                    &mut materials,
                    &color_scheme,
                ));
            }

            for intersection in map_model.all_intersections().iter() {
                map_layer.spawn(IntersectionBundle::new(
                    intersection,
                    map_model,
                    &mut meshes,
                    &mut materials,
                    &color_scheme,
                ));
            }
        });

    let camera_bundle = Camera2dBundle {
        transform: Transform::from_xyz(
            map_bounds.max_x as f32 / 2.,
            -map_bounds.max_y as f32 / 2.,
            1000.,
        ),
        ..default()
    };

    commands
        .spawn(DetailsLayerBundle::default())
        .with_children(|details_layer| {
            for road in map_model.all_roads().iter() {
                if !road.is_light_rail() {
                    for lane in road.lanes.iter() {
                        details_layer.spawn(LaneBundle::new(
                            lane,
                            road,
                            &mut meshes,
                            &mut materials,
                            &color_scheme,
                        ));
                    }
                }
            }
        });

    commands.spawn((
        camera_bundle,
        PickingCameraBundle::default(),
        PanCam::default(),
    ));
}

fn handle_selection(
    mut ev_selection: EventReader<PickingEvent>,
    mut ui_state: ResMut<UiState>,
    lane_ids: Query<&LaneIdComponent>,
) {
    for ev in ev_selection.iter() {
        if let PickingEvent::Selection(selection_event) = ev {
            match selection_event {
                SelectionEvent::JustSelected(entity) => {
                    if let Ok(lane_id) = lane_ids.get_component::<LaneIdComponent>(*entity) {
                        info!("Selected Lane {:?}", lane_id.0);
                        ui_state.selected_lane_id = Some(lane_id.0)
                    }
                }
                SelectionEvent::JustDeselected(_) => ui_state.selected_lane_id = None,
            }
        }
    }
}

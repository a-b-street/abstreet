use crate::objects::ID;
use crate::plugins::{AmbientPlugin, PluginCtx};
use crate::render::MIN_ZOOM_FOR_PARCELS;
use ezgui::ToggleableLayer;

pub struct ToggleableLayers {
    pub show_lanes: ToggleableLayer,
    pub show_buildings: ToggleableLayer,
    pub show_intersections: ToggleableLayer,
    pub show_parcels: ToggleableLayer,
    pub show_extra_shapes: ToggleableLayer,
    pub show_all_turn_icons: ToggleableLayer,
    pub show_areas: ToggleableLayer,
    pub debug_mode: ToggleableLayer,
}

impl ToggleableLayers {
    pub fn new() -> ToggleableLayers {
        ToggleableLayers {
            show_lanes: ToggleableLayer::new("lanes", Some(0.0)),
            show_buildings: ToggleableLayer::new("buildings", Some(0.0)),
            show_intersections: ToggleableLayer::new("intersections", Some(0.0)),
            show_parcels: ToggleableLayer::new("parcels", Some(MIN_ZOOM_FOR_PARCELS)),
            show_extra_shapes: ToggleableLayer::new("extra shapes", Some(0.0)),
            show_all_turn_icons: ToggleableLayer::new("all turn icons", None),
            show_areas: ToggleableLayer::new("areas", Some(0.0)),
            debug_mode: ToggleableLayer::new("geometry debug mode", None),
        }
    }

    pub fn show(&self, id: ID) -> bool {
        match id {
            ID::Road(_) | ID::Lane(_) => self.show_lanes.is_enabled(),
            ID::Building(_) => self.show_buildings.is_enabled(),
            ID::Intersection(_) => self.show_intersections.is_enabled(),
            ID::Parcel(_) => self.show_parcels.is_enabled(),
            ID::ExtraShape(_) => self.show_extra_shapes.is_enabled(),
            ID::Area(_) => self.show_areas.is_enabled(),
            _ => true,
        }
    }

    pub fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64) {
        for layer in self.toggleable_layers().into_iter() {
            layer.handle_zoom(old_zoom, new_zoom);
        }
    }

    fn toggleable_layers(&mut self) -> Vec<&mut ToggleableLayer> {
        vec![
            &mut self.show_lanes,
            &mut self.show_buildings,
            &mut self.show_intersections,
            &mut self.show_parcels,
            &mut self.show_extra_shapes,
            &mut self.show_all_turn_icons,
            &mut self.show_areas,
            &mut self.debug_mode,
        ]
    }
}

impl AmbientPlugin for ToggleableLayers {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        for layer in self.toggleable_layers().into_iter() {
            if layer.event(ctx.input) {
                *ctx.recalculate_current_selection = true;
                ctx.primary.current_selection = None;
                return;
            }
        }
    }
}

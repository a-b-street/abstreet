use crate::objects::ID;
use crate::plugins::{AmbientPlugin, PluginCtx};

pub struct ToggleableLayers {
    pub show_buildings: bool,
    pub show_intersections: bool,
    pub show_lanes: bool,
    pub show_areas: bool,
    pub show_extra_shapes: bool,
    pub show_all_turn_icons: bool,
    pub debug_mode: bool,
}

impl ToggleableLayers {
    pub fn new() -> ToggleableLayers {
        ToggleableLayers {
            show_buildings: true,
            show_intersections: true,
            show_lanes: true,
            show_areas: true,
            show_extra_shapes: true,
            show_all_turn_icons: false,
            debug_mode: false,
        }
    }

    // TODO Probably don't need this later
    pub fn show(&self, id: ID) -> bool {
        match id {
            ID::Road(_) | ID::Lane(_) => self.show_lanes,
            ID::Building(_) => self.show_buildings,
            ID::Intersection(_) => self.show_intersections,
            ID::ExtraShape(_) => self.show_extra_shapes,
            ID::Area(_) => self.show_areas,
            _ => true,
        }
    }
}

impl AmbientPlugin for ToggleableLayers {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        if ctx.input.action_chosen("show/hide buildings") {
            self.show_buildings = !self.show_buildings;
        } else if ctx.input.action_chosen("show/hide intersections") {
            self.show_intersections = !self.show_intersections;
        } else if ctx.input.action_chosen("show/hide lanes") {
            self.show_lanes = !self.show_lanes;
        } else if ctx.input.action_chosen("show/hide areas") {
            self.show_areas = !self.show_areas;
        } else if ctx.input.action_chosen("show/hide extra shapes") {
            self.show_extra_shapes = !self.show_extra_shapes;
        } else if ctx.input.action_chosen("show/hide all turn icons") {
            self.show_all_turn_icons = !self.show_all_turn_icons;
        } else if ctx.input.action_chosen("show/hide geometry debug mode") {
            self.debug_mode = !self.debug_mode;
        } else {
            return;
        }

        *ctx.recalculate_current_selection = true;
        ctx.primary.current_selection = None;
    }
}

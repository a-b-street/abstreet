use crate::objects::{DrawCtx, ID};
use crate::plugins::{NonblockingPlugin, PluginCtx};
use ezgui::{Color, GfxCtx, Key};
use map_model::RoadID;
use std::collections::HashSet;

pub struct ShowOriginalRoads {
    roads: HashSet<RoadID>,
}

impl ShowOriginalRoads {
    pub fn new(ctx: &mut PluginCtx) -> Option<ShowOriginalRoads> {
        if let Some(id) = show_road(ctx) {
            let mut roads = HashSet::new();
            roads.insert(id);
            return Some(ShowOriginalRoads { roads });
        }
        None
    }
}

impl NonblockingPlugin for ShowOriginalRoads {
    fn nonblocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Original Roads", &ctx.canvas);

        if ctx.input.modal_action("quit") {
            return false;
        }

        if let Some(id) = show_road(ctx) {
            self.roads.insert(id);
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        for id in &self.roads {
            let r = ctx.map.get_r(*id);
            if let Some(pair) = r.get_center_for_side(true) {
                let (pl, width) = pair.unwrap();
                g.draw_polygon(
                    ctx.cs
                        .get_def("original road forwards", Color::RED.alpha(0.5)),
                    &pl.make_polygons(width),
                );
            }
            if let Some(pair) = r.get_center_for_side(false) {
                let (pl, width) = pair.unwrap();
                g.draw_polygon(
                    ctx.cs
                        .get_def("original road backwards", Color::BLUE.alpha(0.5)),
                    &pl.make_polygons(width),
                );
            }
        }
    }
}

fn show_road(ctx: &mut PluginCtx) -> Option<RoadID> {
    if let Some(ID::Lane(l)) = ctx.primary.current_selection {
        let id = ctx.primary.map.get_l(l).parent;
        if ctx
            .input
            .contextual_action(Key::V, &format!("show original geometry of {:?}", id))
        {
            return Some(id);
        }
    }
    None
}

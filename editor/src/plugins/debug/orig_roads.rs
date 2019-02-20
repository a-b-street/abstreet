use crate::objects::{DrawCtx, ID};
use crate::plugins::{NonblockingPlugin, PluginCtx};
use ezgui::{Color, GfxCtx, Key};
use geom::Distance;
use map_model::{RoadID, LANE_THICKNESS};
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
            // TODO Should be a less tedious way to do this
            let width_right = (r.children_forwards.len() as f64) * LANE_THICKNESS;
            let width_left = (r.children_backwards.len() as f64) * LANE_THICKNESS;
            if width_right != Distance::ZERO {
                g.draw_polygon(
                    ctx.cs
                        .get_def("original road forwards", Color::RED.alpha(0.5)),
                    &r.original_center_pts
                        .shift_right(width_right / 2.0)
                        .unwrap()
                        .make_polygons(width_right),
                );
            }
            if width_left != Distance::ZERO {
                g.draw_polygon(
                    ctx.cs
                        .get_def("original road backwards", Color::BLUE.alpha(0.5)),
                    &r.original_center_pts
                        .shift_left(width_left / 2.0)
                        .unwrap()
                        .make_polygons(width_left),
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

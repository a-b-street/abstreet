use crate::objects::DrawCtx;
use crate::plugins::{AmbientPlugin, PluginCtx};
use crate::render::{RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};

pub struct ShowMapDiffs {
    active: bool,
}

impl ShowMapDiffs {
    pub fn new() -> ShowMapDiffs {
        ShowMapDiffs { active: false }
    }
}

impl AmbientPlugin for ShowMapDiffs {
    fn ambient_event(&mut self, ctx: &mut PluginCtx) {
        if self.active {
            ctx.input.set_mode("Map Edits Differ", &ctx.canvas);
            if ctx.input.modal_action("quit") {
                self.active = false;
            }
        } else {
            if ctx.input.action_chosen("show map diffs") {
                self.active = true;
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        if !self.active {
            return;
        }

        // TODO Similar to drawing areas with traffic or not -- would be convenient to just supply
        // a set of things to highlight and have something else take care of drawing with detail or
        // not.

        // More generally we might want to show the diff between two edits, but for now, just show
        // diff relative to basemap.
        let edits = ctx.map.get_edits();
        for l in edits.lane_overrides.keys() {
            ctx.draw_map.get_l(*l).draw(
                g,
                RenderOptions {
                    color: Some(ctx.cs.get_def("map diffs", Color::YELLOW)),
                    debug_mode: false,
                },
                ctx,
            );
        }
        for i in edits
            .stop_sign_overrides
            .keys()
            .chain(edits.traffic_signal_overrides.keys())
        {
            ctx.draw_map.get_i(*i).draw(
                g,
                RenderOptions {
                    color: Some(ctx.cs.get("map diffs")),
                    debug_mode: false,
                },
                ctx,
            );
        }
    }
}

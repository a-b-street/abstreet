use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{GfxCtx, Key, Text};
use geom::Pt2D;

pub struct DebugPolygon {
    pts: Vec<Pt2D>,
    current_pt: usize,
}

impl DebugPolygon {
    pub fn new(ctx: &mut PluginCtx) -> Option<DebugPolygon> {
        if let Some(ID::Intersection(id)) = ctx.primary.current_selection {
            if ctx
                .input
                .contextual_action(Key::X, "debug intersection geometry")
            {
                return Some(DebugPolygon {
                    pts: ctx.primary.map.get_i(id).polygon.clone(),
                    current_pt: 0,
                });
            }
        }
        None
    }
}

impl Plugin for DebugPolygon {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Polygon Debugger", &ctx.canvas);
        if ctx.input.modal_action("quit") {
            return false;
        } else if self.current_pt != self.pts.len() - 1 && ctx.input.modal_action("next point") {
            self.current_pt += 1;
        } else if self.current_pt != 0 && ctx.input.modal_action("prev point") {
            self.current_pt -= 1;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        ctx.canvas.draw_text_at(
            g,
            Text::from_line(format!("{}", self.current_pt)),
            self.pts[self.current_pt],
        );
    }
}

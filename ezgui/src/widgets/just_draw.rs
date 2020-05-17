use crate::{
    svg, Drawable, EventCtx, GeomBatch, GfxCtx, RewriteColor, ScreenDims, ScreenPt,
    ScreenRectangle, Text, Widget, WidgetImpl, WidgetOutput,
};
use geom::Polygon;

// Just draw something. A widget just so widgetsing works.
pub struct JustDraw {
    pub(crate) draw: Drawable,

    pub(crate) top_left: ScreenPt,
    pub(crate) dims: ScreenDims,
}

impl JustDraw {
    pub(crate) fn wrap(ctx: &EventCtx, batch: GeomBatch) -> Widget {
        Widget::new(Box::new(JustDraw {
            dims: batch.get_dims(),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        }))
    }

    pub(crate) fn svg(ctx: &EventCtx, filename: String) -> Widget {
        let (batch, bounds) = svg::load_svg(
            ctx.prerender,
            &filename,
            *ctx.prerender.assets.scale_factor.borrow(),
        );
        // TODO The dims will be wrong; it'll only look at geometry, not the padding in the image.
        Widget::new(Box::new(JustDraw {
            dims: ScreenDims::new(bounds.width(), bounds.height()),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        }))
    }
    pub(crate) fn svg_transform(ctx: &EventCtx, filename: &str, rewrite: RewriteColor) -> Widget {
        let (mut batch, bounds) = svg::load_svg(
            ctx.prerender,
            filename,
            *ctx.prerender.assets.scale_factor.borrow(),
        );
        batch.rewrite_color(rewrite);
        // TODO The dims will be wrong; it'll only look at geometry, not the padding in the image.
        Widget::new(Box::new(JustDraw {
            dims: ScreenDims::new(bounds.width(), bounds.height()),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        }))
    }
}

impl WidgetImpl for JustDraw {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _ctx: &mut EventCtx, _output: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}

pub struct DrawWithTooltips {
    draw: Drawable,
    tooltips: Vec<(Polygon, Text)>,

    top_left: ScreenPt,
    dims: ScreenDims,
}

impl DrawWithTooltips {
    pub fn new(ctx: &EventCtx, batch: GeomBatch, tooltips: Vec<(Polygon, Text)>) -> Widget {
        Widget::new(Box::new(DrawWithTooltips {
            dims: batch.get_dims(),
            top_left: ScreenPt::new(0.0, 0.0),
            draw: ctx.upload(batch),
            tooltips,
        }))
    }
}

impl WidgetImpl for DrawWithTooltips {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, _ctx: &mut EventCtx, _output: &mut WidgetOutput) {}

    fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);

        if let Some(cursor) = g.canvas.get_cursor_in_screen_space() {
            if !ScreenRectangle::top_left(self.top_left, self.dims).contains(cursor) {
                return;
            }
            let translated =
                ScreenPt::new(cursor.x - self.top_left.x, cursor.y - self.top_left.y).to_pt();
            // TODO Assume regions are non-overlapping
            for (region, txt) in &self.tooltips {
                if region.contains_pt(translated) {
                    g.draw_mouse_tooltip(txt.clone());
                    return;
                }
            }
        }
    }
}

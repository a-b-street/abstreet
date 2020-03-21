use crate::{
    svg, Drawable, EventCtx, GeomBatch, GfxCtx, RewriteColor, ScreenDims, ScreenPt, Text, Widget,
    WidgetImpl,
};

// Just draw something. A widget just so widgetsing works.
pub struct JustDraw {
    pub(crate) draw: Drawable,

    pub(crate) top_left: ScreenPt,
    pub(crate) dims: ScreenDims,
}

impl JustDraw {
    pub fn wrap(ctx: &EventCtx, batch: GeomBatch) -> Widget {
        Widget::just_draw(JustDraw {
            dims: batch.get_dims(),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        })
    }

    pub fn svg(ctx: &EventCtx, filename: &str) -> Widget {
        let (batch, bounds) = svg::load_svg(ctx.prerender, filename);
        // TODO The dims will be wrong; it'll only look at geometry, not the padding in the image.
        Widget::just_draw(JustDraw {
            dims: ScreenDims::new(bounds.width(), bounds.height()),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        })
    }
    pub fn svg_transform(ctx: &EventCtx, filename: &str, rewrite: RewriteColor) -> Widget {
        let (mut batch, bounds) = svg::load_svg(ctx.prerender, filename);
        batch.rewrite_color(rewrite);
        // TODO The dims will be wrong; it'll only look at geometry, not the padding in the image.
        Widget::just_draw(JustDraw {
            dims: ScreenDims::new(bounds.width(), bounds.height()),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        })
    }

    pub fn text(ctx: &EventCtx, text: Text) -> Widget {
        JustDraw::wrap(ctx, text.render_ctx(ctx))
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}

impl WidgetImpl for JustDraw {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

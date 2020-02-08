use crate::layout::Widget;
use crate::svg;
use crate::{
    Drawable, EventCtx, GeomBatch, GfxCtx, ManagedWidget, RewriteColor, ScreenDims, ScreenPt, Text,
};

// Just draw something. A widget just so layouting works.
pub struct JustDraw {
    pub(crate) draw: Drawable,

    pub(crate) top_left: ScreenPt,
    pub(crate) dims: ScreenDims,
}

impl JustDraw {
    pub fn wrap(ctx: &EventCtx, batch: GeomBatch) -> ManagedWidget {
        ManagedWidget::just_draw(JustDraw {
            dims: batch.get_dims(),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        })
    }

    pub fn image(ctx: &EventCtx, filename: &str) -> ManagedWidget {
        let (color, rect) = ctx.canvas.texture_rect(filename);
        let batch = GeomBatch::from(vec![(color, rect)]);
        JustDraw::wrap(ctx, batch)
    }

    pub fn svg(ctx: &EventCtx, filename: &str) -> ManagedWidget {
        let mut batch = GeomBatch::new();
        let bounds = svg::add_svg(&mut batch, filename);
        // TODO The dims will be wrong; it'll only look at geometry, not the padding in the image.
        ManagedWidget::just_draw(JustDraw {
            dims: ScreenDims::new(bounds.width(), bounds.height()),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        })
    }
    pub fn svg_transform(ctx: &EventCtx, filename: &str, rewrite: RewriteColor) -> ManagedWidget {
        let mut batch = GeomBatch::new();
        let bounds = svg::add_svg(&mut batch, filename);
        batch.rewrite_color(rewrite);
        // TODO The dims will be wrong; it'll only look at geometry, not the padding in the image.
        ManagedWidget::just_draw(JustDraw {
            dims: ScreenDims::new(bounds.width(), bounds.height()),
            draw: ctx.upload(batch),
            top_left: ScreenPt::new(0.0, 0.0),
        })
    }

    pub fn text(ctx: &EventCtx, text: Text) -> ManagedWidget {
        JustDraw::wrap(ctx, text.render_ctx(ctx))
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        g.redraw_at(self.top_left, &self.draw);
    }
}

impl Widget for JustDraw {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

use crate::layout::Widget;
use crate::{Drawable, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt, Text};
use geom::{Distance, Polygon, Pt2D};

// Just draw something. A widget just so layouting works.
pub struct JustDraw {
    draw: Drawable,

    dims: ScreenDims,
    top_left: ScreenPt,
}

impl JustDraw {
    pub fn image(filename: &str, ctx: &EventCtx) -> JustDraw {
        let color = ctx.canvas.texture(filename);
        let (w, h) = color.texture_dims();
        let draw = GeomBatch::from(vec![(
            color,
            Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                Distance::meters(w),
                Distance::meters(h),
            ),
        )])
        .upload(ctx);
        JustDraw {
            draw,
            dims: ScreenDims::new(w, h),
            top_left: ScreenPt::new(0.0, 0.0),
        }
    }

    // TODO I wish this wasn't a separate type...
    pub fn text(text: Text, ctx: &EventCtx) -> JustDrawText {
        JustDrawText {
            dims: ctx.canvas.text_dims(&text),
            text,
            top_left: ScreenPt::new(0.0, 0.0),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork(Pt2D::new(0.0, 0.0), self.top_left, 1.0);
        g.redraw(&self.draw);
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

pub struct JustDrawText {
    text: Text,

    dims: ScreenDims,
    top_left: ScreenPt,
}

impl JustDrawText {
    pub fn draw(&self, g: &mut GfxCtx) {
        g.draw_text_at_screenspace_topleft(&self.text, self.top_left);
    }
}

impl Widget for JustDrawText {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

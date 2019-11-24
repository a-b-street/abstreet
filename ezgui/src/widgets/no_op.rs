use crate::layout::Widget;
use crate::{Drawable, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt};
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

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork(Pt2D::new(0.0, 0.0), self.top_left, 1.0);
        g.redraw(&self.draw);
    }
}

impl Widget for JustDraw {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt, _total_width: f64) {
        self.top_left = top_left;
    }
}

use crate::layout::Widget;
use crate::svg;
use crate::{DrawBoth, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt, Text};

// Just draw something. A widget just so layouting works.
pub struct JustDraw {
    draw: DrawBoth,

    top_left: ScreenPt,
}

impl JustDraw {
    pub fn wrap(draw: DrawBoth) -> JustDraw {
        JustDraw {
            draw,
            top_left: ScreenPt::new(0.0, 0.0),
        }
    }

    pub fn image(filename: &str, ctx: &EventCtx) -> JustDraw {
        let (color, rect) = ctx.canvas.texture_rect(filename);
        let batch = GeomBatch::from(vec![(color, rect)]);
        JustDraw {
            draw: DrawBoth::new(ctx, batch, vec![]),
            top_left: ScreenPt::new(0.0, 0.0),
        }
    }

    pub fn svg(filename: &str, ctx: &EventCtx) -> JustDraw {
        let mut batch = GeomBatch::new();
        let bounds = svg::add_svg(&mut batch, filename);
        let mut draw = DrawBoth::new(ctx, batch, vec![]);
        // TODO The dims will be wrong; it'll only look at geometry, not the padding in the image.
        draw.override_bounds(bounds);
        JustDraw {
            draw,
            top_left: ScreenPt::new(0.0, 0.0),
        }
    }

    pub fn text(text: Text, ctx: &EventCtx) -> JustDraw {
        JustDraw {
            draw: DrawBoth::new(ctx, GeomBatch::new(), vec![(text, ScreenPt::new(0.0, 0.0))]),
            top_left: ScreenPt::new(0.0, 0.0),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.draw.redraw(self.top_left, g);
    }
}

impl Widget for JustDraw {
    fn get_dims(&self) -> ScreenDims {
        self.draw.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

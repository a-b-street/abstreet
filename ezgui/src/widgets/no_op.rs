use crate::layout::Widget;
use crate::{DrawBoth, EventCtx, GeomBatch, GfxCtx, ScreenDims, ScreenPt, Text};
use geom::{Distance, Polygon, Pt2D};

// Just draw something. A widget just so layouting works.
pub struct JustDraw {
    draw: DrawBoth,

    top_left: ScreenPt,
}

impl JustDraw {
    pub fn image(filename: &str, ctx: &EventCtx) -> JustDraw {
        let color = ctx.canvas.texture(filename);
        let dims = color.texture_dims();
        let batch = GeomBatch::from(vec![(
            color,
            Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                Distance::meters(dims.width),
                Distance::meters(dims.height),
            ),
        )]);
        JustDraw {
            draw: DrawBoth::new(ctx, batch, vec![]),
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
        self.draw.draw(self.top_left, g);
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

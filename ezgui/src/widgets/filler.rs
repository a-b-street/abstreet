use crate::{EventCtx, GfxCtx, ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput};

// Doesn't do anything by itself, just used for widgetsing. Something else reaches in, asks for the
// ScreenRectangle to use.
pub struct Filler {
    top_left: ScreenPt,
    dims: ScreenDims,

    square_width_pct: f64,
}

impl Filler {
    /// Creates a square filler, always some percentage of the window width.
    pub fn square_width(ctx: &EventCtx, pct_width: f64) -> Widget {
        Widget::new(Box::new(Filler {
            dims: ScreenDims::new(
                pct_width * ctx.canvas.window_width,
                pct_width * ctx.canvas.window_width,
            ),
            top_left: ScreenPt::new(0.0, 0.0),
            square_width_pct: pct_width,
        }))
    }
}

impl WidgetImpl for Filler {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }

    fn event(&mut self, ctx: &mut EventCtx, _: &mut WidgetOutput) {
        if ctx.input.is_window_resized() {
            self.dims = ScreenDims::new(
                self.square_width_pct * ctx.canvas.window_width,
                self.square_width_pct * ctx.canvas.window_width,
            );
        }
    }
    fn draw(&self, _g: &mut GfxCtx) {}
}

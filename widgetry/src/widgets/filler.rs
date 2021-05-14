use crate::{EventCtx, GfxCtx, ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput};

/// Doesn't do anything by itself, just used for widgetsing. Something else reaches in, asks for the
/// ScreenRectangle to use.
pub struct Filler {
    resize: ResizeRule,
}

enum ResizeRule {
    FixedSize(ScreenDims),

    // (ratio_of_parent_width, parent_width)
    RatioWidthSquare(f64, f64),
}

impl ResizeRule {
    fn dims(&self) -> ScreenDims {
        match self {
            Self::FixedSize(dims) => *dims,
            Self::RatioWidthSquare(pct_width, width) => ScreenDims::square(pct_width * width),
        }
    }
}

impl Filler {
    /// Creates a square filler, always some percentage of the window width.
    pub fn square_width(ctx: &EventCtx, pct_width: f64) -> Widget {
        Widget::new(Box::new(Filler {
            resize: ResizeRule::RatioWidthSquare(pct_width, ctx.canvas.window_width),
        }))
    }

    pub fn fixed_dims(dims: ScreenDims) -> Widget {
        Widget::new(Box::new(Filler {
            resize: ResizeRule::FixedSize(dims),
        }))
    }
}

impl WidgetImpl for Filler {
    fn get_dims(&self) -> ScreenDims {
        self.resize.dims()
    }

    fn set_pos(&mut self, _: ScreenPt) {}

    fn event(&mut self, ctx: &mut EventCtx, _: &mut WidgetOutput) {
        if ctx.input.is_window_resized() {
            if let ResizeRule::RatioWidthSquare(_, ref mut parent_width) = self.resize {
                *parent_width = ctx.canvas.window_width;
            };
        }
    }

    fn draw(&self, _g: &mut GfxCtx) {}
}

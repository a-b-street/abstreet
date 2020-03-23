use crate::{EventCtx, GfxCtx, Outcome, ScreenDims, ScreenPt, Widget, WidgetImpl};

// Doesn't do anything by itself, just used for widgetsing. Something else reaches in, asks for the
// ScreenRectangle to use.
pub struct Filler {
    top_left: ScreenPt,
    dims: ScreenDims,
}

impl Filler {
    pub fn new(dims: ScreenDims) -> Widget {
        Widget::new(Box::new(Filler {
            dims,
            top_left: ScreenPt::new(0.0, 0.0),
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

    fn event(&mut self, _ctx: &mut EventCtx, _redo_layout: &mut bool) -> Option<Outcome> {
        None
    }
    fn draw(&self, _g: &mut GfxCtx) {}
}

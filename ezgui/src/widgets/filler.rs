use crate::layout::Widget;
use crate::{ScreenDims, ScreenPt};

// Doesn't do anything by itself, just used for layouting. Something else reaches in, asks for the
// ScreenRectangle to use.
pub struct Filler {
    pub(crate) top_left: ScreenPt,
    pub(crate) dims: ScreenDims,
}

impl Filler {
    pub fn new(dims: ScreenDims) -> Filler {
        Filler {
            dims,
            top_left: ScreenPt::new(0.0, 0.0),
        }
    }
}

impl Widget for Filler {
    fn get_dims(&self) -> ScreenDims {
        self.dims
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.top_left = top_left;
    }
}

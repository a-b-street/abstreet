use crate::widgets::Widget;
use crate::{Button, EventCtx, GfxCtx, ScreenDims, ScreenPt};

pub struct Checkbox {
    pub(crate) enabled: bool,
    btn: Button,
    other_btn: Button,
}

impl Checkbox {
    pub fn new(enabled: bool, false_btn: Button, true_btn: Button) -> Checkbox {
        if enabled {
            Checkbox {
                enabled,
                btn: true_btn,
                other_btn: false_btn,
            }
        } else {
            Checkbox {
                enabled,
                btn: false_btn,
                other_btn: true_btn,
            }
        }
    }

    // If true, widgets should be recomputed.
    pub(crate) fn event(&mut self, ctx: &mut EventCtx) -> bool {
        self.btn.event(ctx);
        if self.btn.clicked() {
            std::mem::swap(&mut self.btn, &mut self.other_btn);
            self.btn.set_pos(self.other_btn.top_left);
            self.enabled = !self.enabled;
            true
        } else {
            false
        }
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
    }
}

impl Widget for Checkbox {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }
}

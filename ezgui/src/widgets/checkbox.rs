use crate::{Button, EventCtx, GfxCtx, Outcome, ScreenDims, ScreenPt, ScreenRectangle, WidgetImpl};

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
}

impl WidgetImpl for Checkbox {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }

    fn event(
        &mut self,
        ctx: &mut EventCtx,
        rect: &ScreenRectangle,
        redo_layout: &mut bool,
    ) -> Option<Outcome> {
        // TODO Lying about the rectangle
        if self.btn.event(ctx, rect, redo_layout).is_some() {
            std::mem::swap(&mut self.btn, &mut self.other_btn);
            self.btn.set_pos(self.other_btn.top_left);
            self.enabled = !self.enabled;
            *redo_layout = true;
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
    }
}

use crate::{
    Btn, Button, EventCtx, GfxCtx, MultiKey, Outcome, ScreenDims, ScreenPt, Widget, WidgetImpl,
};

pub struct Checkbox {
    pub(crate) enabled: bool,
    btn: Button,
    other_btn: Button,
}

impl Checkbox {
    // TODO Not typesafe! Gotta pass a button.
    pub fn new(enabled: bool, false_btn: Widget, true_btn: Widget) -> Widget {
        if enabled {
            Widget::new(Box::new(Checkbox {
                enabled,
                btn: true_btn.take_btn(),
                other_btn: false_btn.take_btn(),
            }))
        } else {
            Widget::new(Box::new(Checkbox {
                enabled,
                btn: false_btn.take_btn(),
                other_btn: true_btn.take_btn(),
            }))
        }
    }

    pub fn text(ctx: &EventCtx, label: &str, hotkey: Option<MultiKey>, enabled: bool) -> Widget {
        Checkbox::new(
            enabled,
            Btn::text_fg(format!("[ ] {}", label)).build(ctx, label, hotkey.clone()),
            Btn::text_fg(format!("[X] {}", label)).build(ctx, label, hotkey),
        )
        .outline(ctx.style().outline_thickness, ctx.style().outline_color)
        .named(label)
    }
}

impl WidgetImpl for Checkbox {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }

    fn event(&mut self, ctx: &mut EventCtx, redo_layout: &mut bool) -> Option<Outcome> {
        if self.btn.event(ctx, redo_layout).is_some() {
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

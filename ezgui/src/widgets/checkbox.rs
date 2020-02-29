use crate::layout::Widget;
use crate::{Button, Color, EventCtx, GfxCtx, Line, MultiKey, ScreenDims, ScreenPt, Text};

pub struct Checkbox {
    pub(crate) enabled: bool,
    btn: Button,
    other_btn: Button,
}

impl Checkbox {
    pub fn new(ctx: &EventCtx, label: &str, hotkey: Option<MultiKey>, enabled: bool) -> Checkbox {
        Checkbox {
            enabled,
            btn: make_btn(ctx, label, hotkey.clone(), enabled),
            other_btn: make_btn(ctx, label, hotkey, !enabled),
        }
    }

    pub(crate) fn event(&mut self, ctx: &mut EventCtx) {
        self.btn.event(ctx);
        if self.btn.clicked() {
            std::mem::swap(&mut self.btn, &mut self.other_btn);
            self.btn.set_pos(self.other_btn.top_left);
            self.enabled = !self.enabled;
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

// TODO Just a copy of WrappedComposite::nice_text_button essentially...
fn make_btn(ctx: &EventCtx, label: &str, hotkey: Option<MultiKey>, enabled: bool) -> Button {
    let txt = Text::from(Line(format!(
        "{} {}",
        if enabled { "☑" } else { "☐" },
        label
    )));
    Button::text_no_bg(
        txt.clone(),
        txt.change_fg(Color::ORANGE),
        hotkey,
        label,
        true,
        ctx,
    )
}

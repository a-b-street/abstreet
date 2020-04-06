use crate::{
    Btn, Button, EventCtx, GfxCtx, MultiKey, ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput,
};

pub struct Checkbox {
    pub(crate) enabled: bool,
    btn: Button,
    other_btn: Button,

    // TODO Biiiit of a hack. If Plot could embed a Composite, that'd actually work better.
    cb_to_plot: Option<(String, String)>,
}

impl Checkbox {
    // TODO Not typesafe! Gotta pass a button. Also, make sure to give an ID.
    pub fn new(enabled: bool, false_btn: Widget, true_btn: Widget) -> Widget {
        if enabled {
            Widget::new(Box::new(Checkbox {
                enabled,
                btn: true_btn.take_btn(),
                other_btn: false_btn.take_btn(),
                cb_to_plot: None,
            }))
        } else {
            Widget::new(Box::new(Checkbox {
                enabled,
                btn: false_btn.take_btn(),
                other_btn: true_btn.take_btn(),
                cb_to_plot: None,
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

    pub(crate) fn callback_to_plot(mut self, plot_id: &str, checkbox_label: &str) -> Checkbox {
        self.cb_to_plot = Some((plot_id.to_string(), checkbox_label.to_string()));
        self
    }
}

impl WidgetImpl for Checkbox {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        self.btn.event(ctx, output);
        if output.outcome.take().is_some() {
            std::mem::swap(&mut self.btn, &mut self.other_btn);
            self.btn.set_pos(self.other_btn.top_left);
            self.enabled = !self.enabled;
            output.redo_layout = true;
            if let Some(ref pair) = self.cb_to_plot {
                output.plot_changed.push((pair.clone(), self.enabled));
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
    }

    fn can_restore(&self) -> bool {
        // TODO I'm nervous about doing this one in general, so just do it for plot checkboxes.
        self.cb_to_plot.is_some()
    }
    fn restore(&mut self, _: &mut EventCtx, prev: &Box<dyn WidgetImpl>) {
        let prev = prev.downcast_ref::<Checkbox>().unwrap();
        if self.enabled != prev.enabled {
            std::mem::swap(&mut self.btn, &mut self.other_btn);
            self.btn.set_pos(self.other_btn.top_left);
            self.enabled = !self.enabled;
        }
    }
}

use crate::{EventCtx, GfxCtx, ScreenDims, ScreenPt, Widget, WidgetImpl, Line, Outcome, WidgetOutput};

/// Delegates to either an "expanded" or "minimized" widget.
pub struct CollapsibleSection {
    full: Widget,
    minimized: Widget,

    is_minimized: bool,
}

impl CollapsibleSection {
    pub fn new(ctx: &mut EventCtx, label: &str, full: Widget) -> Widget {
        let minimized = Widget::row(vec![
            Line(label).small_heading().into_widget(ctx),
            ctx.style().btn_plain.text("Show").build_def(ctx),
        ]);
        let full = Widget::col(vec![Widget::row(vec![
            Line(label).small_heading().into_widget(ctx),
            ctx.style().btn_plain.text("Hide").build_def(ctx),
        ]), full]);

        Widget::new(Box::new(CollapsibleSection {
            full,
            minimized,
            is_minimized: false,
        }))
    }
}

impl WidgetImpl for CollapsibleSection {
    fn get_dims(&self) -> ScreenDims {
        if self.is_minimized {
            self.minimized.widget.get_dims()
        } else {
            self.full.widget.get_dims()
        }
    }

    fn set_pos(&mut self, pt: ScreenPt) {
        if self.is_minimized {
            //self.minimized.widget.set_pos(pt);
        } else {
            //self.full.widget.set_pos(pt);
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        if self.is_minimized {
            self.minimized.widget.event(ctx, output);
        } else {
            self.full.widget.event(ctx, output);
        }
        if let Outcome::Clicked(ref x) = output.outcome {
            if x == "show" {
                self.is_minimized = false;
                output.outcome = Outcome::Nothing;
            } else if x == "hide" {
                self.is_minimized = true;
                output.outcome = Outcome::Nothing;
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        if self.is_minimized {
            self.minimized.widget.draw(g);
        } else {
            self.full.widget.draw(g);
        }
    }
}

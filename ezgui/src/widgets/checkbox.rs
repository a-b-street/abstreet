use crate::{
    Btn, Button, Color, EventCtx, GeomBatch, GfxCtx, Line, MultiKey, Outcome, RewriteColor,
    ScreenDims, ScreenPt, Text, TextExt, TextSpan, Widget, WidgetImpl, WidgetOutput,
};

pub struct Checkbox {
    pub(crate) enabled: bool,
    btn: Button,
    other_btn: Button,
}

impl Checkbox {
    // TODO Not typesafe! Gotta pass a button. Also, make sure to give an ID.
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

    pub fn switch<I: Into<String>>(
        ctx: &EventCtx,
        label: I,
        hotkey: Option<MultiKey>,
        enabled: bool,
    ) -> Widget {
        let label = label.into();
        let (off, hitbox) = Widget::row(vec![
            GeomBatch::screenspace_svg(ctx.prerender, "system/assets/tools/toggle_off.svg")
                .batch()
                .centered_vert(),
            label.clone().batch_text(ctx),
        ])
        .to_geom(ctx, None);
        let (on, _) = Widget::row(vec![
            GeomBatch::screenspace_svg(ctx.prerender, "system/assets/tools/toggle_on.svg")
                .batch()
                .centered_vert(),
            label.clone().batch_text(ctx),
        ])
        .to_geom(ctx, None);

        Checkbox::new(
            enabled,
            Btn::custom(
                off.clone(),
                off.color(RewriteColor::Change(
                    Color::hex("#F2F2F2"),
                    ctx.style().hovering_color,
                )),
                hitbox.clone(),
            )
            .build(ctx, &label, hotkey.clone()),
            Btn::custom(
                on.clone(),
                on.color(RewriteColor::Change(
                    Color::hex("#F2F2F2"),
                    ctx.style().hovering_color,
                )),
                hitbox,
            )
            .build(ctx, &label, hotkey),
        )
        .named(label)
    }

    pub fn checkbox<I: Into<String>>(
        ctx: &EventCtx,
        label: I,
        hotkey: Option<MultiKey>,
        enabled: bool,
    ) -> Widget {
        let label = label.into();
        Checkbox::new(
            enabled,
            Btn::text_fg(format!("[ ] {}", label)).build(ctx, &label, hotkey.clone()),
            Btn::text_fg(format!("[X] {}", label)).build(ctx, &label, hotkey),
        )
        .outline(ctx.style().outline_thickness, ctx.style().outline_color)
        .named(label)
    }

    pub fn custom_checkbox<I: Into<String>>(
        ctx: &EventCtx,
        label: I,
        spans: Vec<TextSpan>,
        hotkey: Option<MultiKey>,
        enabled: bool,
    ) -> Widget {
        let label = label.into();
        let mut off = vec![Line("[ ] ")];
        let mut on = vec![Line("[X] ")];
        off.extend(spans.clone());
        on.extend(spans);

        Checkbox::new(
            enabled,
            Btn::txt(&label, Text::from_all(off)).build_def(ctx, hotkey.clone()),
            Btn::txt(&label, Text::from_all(on)).build_def(ctx, hotkey),
        )
        .outline(ctx.style().outline_thickness, ctx.style().outline_color)
        .named(label)
    }

    pub fn colored(ctx: &EventCtx, label: &str, color: Color, enabled: bool) -> Widget {
        let (off, hitbox) = Widget::row(vec![
            GeomBatch::screenspace_svg(ctx.prerender, "system/assets/tools/checkbox.svg")
                .color(RewriteColor::ChangeAll(color.alpha(0.3)))
                .batch()
                .centered_vert(),
            label.batch_text(ctx),
        ])
        .to_geom(ctx, None);
        let (on, _) = Widget::row(vec![
            GeomBatch::screenspace_svg(ctx.prerender, "system/assets/tools/checkbox.svg")
                .color(RewriteColor::Change(Color::BLACK, color))
                .batch()
                .centered_vert(),
            label.batch_text(ctx),
        ])
        .to_geom(ctx, None);

        Checkbox::new(
            enabled,
            Btn::custom(
                off.clone(),
                off.color(RewriteColor::Change(
                    Color::WHITE,
                    ctx.style().hovering_color,
                )),
                hitbox.clone(),
            )
            .build(ctx, label, None),
            Btn::custom(
                on.clone(),
                on.color(RewriteColor::Change(
                    Color::WHITE,
                    ctx.style().hovering_color,
                )),
                hitbox,
            )
            .build(ctx, label, None),
        )
        .named(label)
    }

    // TODO These should actually be radio buttons
    pub fn toggle<I: Into<String>>(
        ctx: &EventCtx,
        label: I,
        left_label: I,
        right_label: I,
        hotkey: Option<MultiKey>,
        enabled: bool,
    ) -> Widget {
        let left_label = left_label.into();
        let right_label = right_label.into();
        Widget::row(vec![
            left_label.clone().draw_text(ctx),
            Checkbox::new(
                enabled,
                Btn::svg_def("system/assets/tools/toggle_right.svg").build(
                    ctx,
                    left_label,
                    hotkey.clone(),
                ),
                Btn::svg_def("system/assets/tools/toggle_left.svg").build(
                    ctx,
                    right_label.clone(),
                    hotkey,
                ),
            )
            .named(label),
            right_label.draw_text(ctx),
        ])
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
        if let Outcome::Clicked(_) = output.outcome {
            output.outcome = Outcome::Nothing;
            std::mem::swap(&mut self.btn, &mut self.other_btn);
            self.btn.set_pos(self.other_btn.top_left);
            self.enabled = !self.enabled;
            output.redo_layout = true;
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
    }
}

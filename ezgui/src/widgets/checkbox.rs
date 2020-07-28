use crate::{
    Btn, Button, Color, EventCtx, GeomBatch, GfxCtx, Line, MultiKey, RewriteColor, ScreenDims,
    ScreenPt, Text, TextExt, TextSpan, Widget, WidgetImpl, WidgetOutput,
};
use geom::{Polygon, Pt2D, Ring};

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
        let off = icon_and_text(ctx, "system/assets/tools/toggle_off.svg", &label);
        let on = icon_and_text(ctx, "system/assets/tools/toggle_on.svg", &label);
        // The dims should be the same for both
        let dims = off.get_dims();
        let vert_pad = 4.0;
        let horiz_pad = 4.0;
        let hitbox = Polygon::rectangle(dims.width + 2.0 * horiz_pad, dims.height + 2.0 * vert_pad);
        let off = off.translate(horiz_pad, vert_pad);
        let on = on.translate(horiz_pad, vert_pad);

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
        //.outline(ctx.style().outline_thickness, ctx.style().outline_color)
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
        let vert_pad = 4.0;
        let horiz_pad = 4.0;

        // TODO What was I thinking...
        let checkmark = Ring::must_new(vec![
            Pt2D::new(11.4528, 22.1072),
            Pt2D::new(5.89284, 16.5472),
            Pt2D::new(3.99951, 18.4272),
            Pt2D::new(11.4528, 25.8805),
            Pt2D::new(27.4528, 9.88049),
            Pt2D::new(25.5728, 8.00049),
            Pt2D::new(11.4528, 22.1072),
        ])
        .to_polygon()
        .translate(0.0, -4.0);
        let bounds = checkmark.get_bounds();
        let hitbox = Polygon::rectangle(
            bounds.width() + 2.0 * horiz_pad,
            bounds.height() + 2.0 * vert_pad,
        );

        let true_btn = Btn::custom(
            GeomBatch::from(vec![
                (color, hitbox.clone()),
                (Color::WHITE, checkmark.clone()),
            ]),
            GeomBatch::from(vec![
                (color, hitbox.clone()),
                (ctx.style().hovering_color, checkmark),
            ]),
            hitbox.clone(),
        )
        .build(ctx, format!("hide {}", label), None);

        let false_btn = Btn::custom(
            GeomBatch::from(vec![(color.alpha(0.3), hitbox.clone())]),
            GeomBatch::from(vec![(color, hitbox.clone())]),
            hitbox,
        )
        .build(ctx, format!("show {}", label), None);

        Checkbox::new(enabled, false_btn, true_btn).named(label)
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
        if output.outcome.take().is_some() {
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

// TODO This should become a BtnBuilder style
fn icon_and_text(ctx: &EventCtx, svg_path: &str, label: &str) -> GeomBatch {
    let mut batch = GeomBatch::screenspace_svg(ctx.prerender, svg_path);
    let horiz_pad = 8.0;
    let txt = Text::from(Line(label))
        .render_to_batch(ctx.prerender)
        .translate(batch.get_dims().width + horiz_pad, 0.0);
    batch.append(txt);
    batch
}

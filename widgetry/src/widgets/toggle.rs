use crate::svg::load_svg_bytes;
use crate::{
    include_labeled_bytes, Button, Color, ControlState, EdgeInsets, EventCtx, GfxCtx, MultiKey,
    Outcome, RewriteColor, ScreenDims, ScreenPt, StyledButtons, Text, TextSpan, Widget, WidgetImpl,
    WidgetOutput,
};

pub struct Toggle {
    pub(crate) enabled: bool,
    pub(crate) btn: Button,
    other_btn: Button,
}

impl Toggle {
    pub fn new(enabled: bool, false_btn: Button, true_btn: Button) -> Widget {
        if enabled {
            Widget::new(Box::new(Toggle {
                enabled,
                btn: true_btn,
                other_btn: false_btn,
            }))
        } else {
            Widget::new(Box::new(Toggle {
                enabled,
                btn: false_btn,
                other_btn: true_btn,
            }))
        }
    }

    pub fn switch<MK: Into<Option<MultiKey>>>(
        ctx: &EventCtx,
        label: &str,
        hotkey: MK,
        enabled: bool,
    ) -> Widget {
        let mut buttons = ctx
            .style()
            .btn_plain_text(label)
            // we don't want the default coloring, because we do custom coloring below
            .image_color(RewriteColor::NoOp, ControlState::Default);

        if let Some(hotkey) = hotkey.into() {
            buttons = buttons.hotkey(hotkey);
        }

        let (off_batch, off_bounds) = {
            let (label, bytes) = include_labeled_bytes!("../../icons/switch_off.svg");
            let (batch, bounds) = load_svg_bytes(ctx.prerender, label, bytes).expect("invalid SVG");
            let batch = batch
                .color(RewriteColor::Change(Color::WHITE, ctx.style.btn_solid.bg))
                .color(RewriteColor::Change(Color::BLACK, ctx.style.btn_solid.fg));
            (batch, bounds)
        };
        let (on_batch, on_bounds) = {
            let (label, bytes) = include_labeled_bytes!("../../icons/switch_on.svg");
            let (batch, bounds) = load_svg_bytes(ctx.prerender, label, bytes).expect("invalid SVG");
            let batch = batch
                .color(RewriteColor::Change(Color::WHITE, ctx.style.btn_solid.bg))
                .color(RewriteColor::Change(Color::BLACK, ctx.style.btn_solid.fg));
            (batch, bounds)
        };

        let off_button = buttons
            .clone()
            .image_batch(off_batch, off_bounds)
            .build(ctx, label);

        let on_button = buttons.image_batch(on_batch, on_bounds).build(ctx, label);

        Toggle::new(enabled, off_button, on_button).named(label)
    }

    pub fn checkbox<MK: Into<Option<MultiKey>>>(
        ctx: &EventCtx,
        label: &str,
        hotkey: MK,
        enabled: bool,
    ) -> Widget {
        let mut false_btn = ctx
            .style()
            .btn_plain_icon_bytes(include_labeled_bytes!("../../icons/checkbox_unchecked.svg"))
            .image_color(
                RewriteColor::Change(Color::BLACK, ctx.style().btn_outline.outline),
                ControlState::Default,
            )
            .image_color(
                RewriteColor::Change(Color::BLACK, ctx.style().btn_outline.outline),
                ControlState::Hovered,
            )
            .image_color(
                RewriteColor::Change(Color::BLACK, ctx.style().btn_outline.outline),
                ControlState::Disabled,
            )
            .label_text(label);

        if let Some(hotkey) = hotkey.into() {
            false_btn = false_btn.hotkey(hotkey);
        }

        let true_btn = false_btn
            .clone()
            .image_bytes(include_labeled_bytes!("../../icons/checkbox_checked.svg"));

        Toggle::new(
            enabled,
            false_btn.build(ctx, label),
            true_btn.build(ctx, label),
        )
        .named(label)
    }

    pub fn custom_checkbox<MK: Into<Option<MultiKey>>>(
        ctx: &EventCtx,
        action: &str,
        spans: Vec<TextSpan>,
        hotkey: MK,
        enabled: bool,
    ) -> Widget {
        let mut false_btn = ctx
            .style()
            .btn_plain_icon_bytes(include_labeled_bytes!("../../icons/checkbox_unchecked.svg"))
            .image_color(
                RewriteColor::Change(Color::BLACK, ctx.style().btn_outline.outline),
                ControlState::Default,
            )
            .image_color(
                RewriteColor::Change(Color::BLACK, ctx.style().btn_outline.outline),
                ControlState::Hovered,
            )
            .image_color(
                RewriteColor::Change(Color::BLACK, ctx.style().btn_outline.outline),
                ControlState::Disabled,
            )
            .label_styled_text(Text::from_all(spans), ControlState::Default);

        if let Some(hotkey) = hotkey.into() {
            false_btn = false_btn.hotkey(hotkey);
        }

        let true_btn = false_btn
            .clone()
            .image_bytes(include_labeled_bytes!("../../icons/checkbox_checked.svg"));

        Toggle::new(
            enabled,
            false_btn.build(ctx, action),
            true_btn.build(ctx, action),
        )
        .named(action)
    }

    pub fn colored_checkbox(ctx: &EventCtx, label: &str, color: Color, enabled: bool) -> Widget {
        let buttons = ctx.style().btn_plain().label_text(label).padding(4.0);

        let false_btn = buttons
            .clone()
            .image_bytes(include_labeled_bytes!(
                "../../icons/checkbox_no_border_unchecked.svg"
            ))
            .image_color(
                RewriteColor::Change(Color::BLACK, color.alpha(0.3)),
                ControlState::Default,
            );

        let true_btn = buttons
            .image_bytes(include_labeled_bytes!(
                "../../icons/checkbox_no_border_checked.svg"
            ))
            .image_color(
                RewriteColor::Change(Color::BLACK, color),
                ControlState::Default,
            );

        Toggle::new(
            enabled,
            false_btn.build(ctx, label),
            true_btn.build(ctx, label),
        )
        .named(label)
    }

    // TODO These should actually be radio buttons
    pub fn choice<MK: Into<Option<MultiKey>>>(
        ctx: &EventCtx,
        label: &str,
        left_label: &str,
        right_label: &str,
        hotkey: MK,
        enabled: bool,
    ) -> Widget {
        let mut toggle_left_button = ctx
            .style()
            .btn_plain()
            .image_dims(ScreenDims::new(40.0, 40.0))
            .padding(4)
            // we don't want the default coloring, because we do custom coloring below
            .image_color(RewriteColor::NoOp, ControlState::Default);

        if let Some(hotkey) = hotkey.into() {
            toggle_left_button = toggle_left_button.hotkey(hotkey);
        }

        let (left_batch, left_bounds) = {
            let (label, bytes) = include_labeled_bytes!("../../icons/toggle_left.svg");
            let (batch, bounds) = load_svg_bytes(ctx.prerender, label, bytes).expect("invalid SVG");
            let batch = batch
                .color(RewriteColor::Change(Color::WHITE, ctx.style.btn_solid.bg))
                .color(RewriteColor::Change(Color::BLACK, ctx.style.btn_solid.fg));
            (batch, bounds)
        };
        let (right_batch, right_bounds) = {
            let (label, bytes) = include_labeled_bytes!("../../icons/toggle_right.svg");
            let (batch, bounds) = load_svg_bytes(ctx.prerender, label, bytes).expect("invalid SVG");
            let batch = batch
                .color(RewriteColor::Change(Color::WHITE, ctx.style.btn_solid.bg))
                .color(RewriteColor::Change(Color::BLACK, ctx.style.btn_solid.fg));
            (batch, bounds)
        };

        let toggle_right_button = toggle_left_button
            .clone()
            .image_batch(right_batch, right_bounds);

        let toggle_left_button = toggle_left_button
            .clone()
            .image_batch(left_batch, left_bounds);

        let left_text_button = ctx
            .style()
            .btn_plain_text(left_label)
            // Cheat vertical padding to align with switch
            .padding(EdgeInsets {
                left: 2.0,
                right: 2.0,
                top: 8.0,
                bottom: 14.0,
            })
            // TODO: make these clickable. Currently they would explode due to re-use of an action
            .disabled(true)
            .label_color(ctx.style().btn_outline.fg, ControlState::Disabled)
            .bg_color(Color::CLEAR, ControlState::Disabled);
        let right_text_button = left_text_button.clone().label_text(right_label);
        Widget::row(vec![
            left_text_button.build_def(ctx).centered_vert(),
            Toggle::new(
                enabled,
                toggle_right_button.build(ctx, right_label),
                toggle_left_button.build(ctx, left_label),
            )
            .named(label)
            .centered_vert(),
            right_text_button.build_def(ctx).centered_vert(),
        ])
    }
}

impl WidgetImpl for Toggle {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        self.btn.event(ctx, output);
        if let Outcome::Clicked(_) = output.outcome {
            output.outcome = Outcome::Changed;
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

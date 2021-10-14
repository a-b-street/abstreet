use geom::Polygon;

use crate::{
    Button, ButtonBuilder, Choice, Color, ControlState, Dropdown, EventCtx, GeomBatch, GfxCtx,
    JustDraw, MultiKey, Outcome, ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput,
};

// TODO Radio buttons in the menu
pub struct PersistentSplit<T: Clone + PartialEq> {
    current_value: T,
    btn: Button,
    spacer: JustDraw,
    dropdown: Dropdown<T>,
}

impl<T: 'static + PartialEq + Clone + std::fmt::Debug> PersistentSplit<T> {
    pub fn widget<MK: Into<Option<MultiKey>>>(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        hotkey: MK,
        choices: Vec<Choice<T>>,
    ) -> Widget {
        let outline = ctx.style().btn_outline.outline;
        Widget::new(Box::new(PersistentSplit::new(
            ctx,
            label,
            default_value,
            hotkey,
            choices,
        )))
        .outline(outline)
        .named(label)
    }

    pub fn new<MK: Into<Option<MultiKey>>>(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        hotkey: MK,
        choices: Vec<Choice<T>>,
    ) -> PersistentSplit<T> {
        let dropdown = Dropdown::new(ctx, "change", default_value, choices, true);
        let mut btn = button_builder(ctx).label_text(dropdown.current_value_label());

        if let Some(multikey) = hotkey.into() {
            btn = btn.hotkey(multikey)
        }
        let btn = btn.build(ctx, label);

        let outline_style = &ctx.style().btn_outline;
        let (_, outline_color) = outline_style.outline;

        PersistentSplit {
            current_value: dropdown.current_value(),
            spacer: JustDraw::wrap(
                ctx,
                GeomBatch::from(vec![(
                    outline_color,
                    Polygon::rectangle(3.0, btn.get_dims().height),
                )]),
            )
            .take_just_draw(),
            btn,
            dropdown,
        }
    }
}

fn button_builder<'a, 'c>(ctx: &EventCtx) -> ButtonBuilder<'a, 'c> {
    ctx.style()
        .btn_plain
        .btn()
        .outline((0.0, Color::CLEAR), ControlState::Default)
}

impl<T: 'static + PartialEq + Clone> PersistentSplit<T> {
    pub fn current_value(&self) -> T {
        self.current_value.clone()
    }
}

impl<T: 'static + Clone + PartialEq> WidgetImpl for PersistentSplit<T> {
    fn get_dims(&self) -> ScreenDims {
        let dims1 = self.btn.get_dims();
        let dims2 = self.spacer.get_dims();
        let dims3 = self.dropdown.get_dims();
        ScreenDims::new(
            dims1.width + dims2.width + dims3.width,
            dims1.height.max(dims2.height).max(dims3.height),
        )
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
        self.spacer
            .set_pos(ScreenPt::new(top_left.x + self.btn.dims.width, top_left.y));
        self.dropdown.set_pos(ScreenPt::new(
            top_left.x + self.btn.dims.width + self.spacer.get_dims().width,
            top_left.y,
        ));
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        self.btn.event(ctx, output);
        if let Outcome::Clicked(_) = output.outcome {
            return;
        }

        let mut tmp_output = WidgetOutput::new();
        self.dropdown.event(ctx, &mut tmp_output);
        if tmp_output.current_focus_owned_by.is_some() {
            // The dropdown's label is a dummy value
            output.steal_focus(self.btn.action.clone());
        }

        let new_value = self.dropdown.current_value();
        if new_value != self.current_value {
            self.current_value = new_value;
            let label = self.btn.action.clone();
            let mut button_builder =
                button_builder(ctx).label_text(self.dropdown.current_value_label());
            if let Some(multikey) = self.btn.hotkey.take() {
                button_builder = button_builder.hotkey(multikey)
            }
            self.btn = button_builder.build(ctx, &label);
            output.redo_layout = true;
            output.outcome = Outcome::Changed(label);
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
        self.spacer.draw(g);
        self.dropdown.draw(g);
    }
}

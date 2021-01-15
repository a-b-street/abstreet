use geom::Polygon;

use crate::{
    Button, ButtonBuilder, Choice, Color, Dropdown, EventCtx, GeomBatch, GfxCtx, JustDraw,
    MultiKey, Outcome, ScreenDims, ScreenPt, Widget, WidgetImpl, WidgetOutput,
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
        let bg = Color::hex("#003046").alpha(0.8);
        let outline = Color::hex("#003046").alpha(0.6);
        Widget::new(Box::new(PersistentSplit::new(
            ctx,
            label,
            default_value,
            hotkey,
            choices,
        )))
        .bg(bg)
        .outline(2.0, outline)
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
        let mut btn = button_builder().label_text(dropdown.current_value_label());

        if let Some(multikey) = hotkey.into() {
            btn = btn.hotkey(multikey)
        }
        let btn = btn.build(ctx, label);

        let outline = Color::hex("#003046").alpha(0.6);

        PersistentSplit {
            current_value: dropdown.current_value(),
            spacer: JustDraw::wrap(
                ctx,
                GeomBatch::from(vec![(
                    outline,
                    Polygon::rectangle(3.0, btn.get_dims().height),
                )]),
            )
            .take_just_draw(),
            btn,
            dropdown,
        }
    }
}

// TODO: It'd be nice for this to be configurable.
//
// I'd like to base it on ColorScheme, but that currently lives in map_gui, so for now
// I've hardcoded the builder colors. We literally use it in one place.
fn button_builder<'a>() -> ButtonBuilder<'a> {
    // let primary_light = ButtonColorScheme {
    //     fg: hex("#F2F2F2"),
    //     fg_disabled: hex("#F2F2F2").alpha(0.3),
    //     bg: hex("#003046").alpha(0.8),
    //     bg_hover: hex("#003046"),
    //     bg_disabled: Color::grey(0.1),
    //     outline: hex("#003046").alpha(0.6),
    // };
    let fg = Color::hex("#F2F2F2");
    let fg_disabled = Color::hex("#F2F2F2").alpha(0.3);
    let bg_hover = Color::hex("#003046");
    let bg_disabled = Color::grey(0.1);

    use crate::ButtonState;
    ButtonBuilder::new()
        .font_size(18)
        .label_color(fg, ButtonState::Default)
        .label_color(fg_disabled, ButtonState::Disabled)
        .bg_color(bg_hover, ButtonState::Hover)
        .bg_color(bg_disabled, ButtonState::Disabled)
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

        self.dropdown.event(ctx, &mut WidgetOutput::new());
        let new_value = self.dropdown.current_value();
        if new_value != self.current_value {
            self.current_value = new_value;
            let label = self.btn.action.clone();
            let mut button_builder =
                button_builder().label_text(self.dropdown.current_value_label());
            if let Some(multikey) = self.btn.hotkey.take() {
                button_builder = button_builder.hotkey(multikey)
            }
            self.btn = button_builder.build(ctx, &label);
            output.redo_layout = true;
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
        self.spacer.draw(g);
        self.dropdown.draw(g);
    }
}

use geom::{Distance, Polygon, Pt2D};

use crate::{
    Button, ButtonBuilder, ButtonState, Choice, Color, EdgeInsets, EventCtx, GeomBatch, GfxCtx,
    Menu, Outcome, ScreenDims, ScreenPt, ScreenRectangle, WidgetImpl, WidgetOutput,
};

pub struct Dropdown<T: Clone> {
    current_idx: usize,
    btn: Button,
    // TODO Why not T?
    menu: Option<Menu<usize>>,
    label: String,
    is_persisten_split: bool,

    choices: Vec<Choice<T>>,
}

impl<T: 'static + PartialEq + Clone + std::fmt::Debug> Dropdown<T> {
    pub fn new(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        choices: Vec<Choice<T>>,
        // TODO Ideally builder style
        is_persisten_split: bool,
    ) -> Dropdown<T> {
        let current_idx = if let Some(idx) = choices.iter().position(|c| c.data == default_value) {
            idx
        } else {
            panic!(
                "Dropdown {} has default_value {:?}, but none of the choices match that",
                label, default_value
            );
        };

        Dropdown {
            current_idx,
            btn: make_btn(ctx, &choices[current_idx].label, label, is_persisten_split),
            menu: None,
            label: label.to_string(),
            is_persisten_split,
            choices,
        }
    }
}

impl<T: 'static + PartialEq + Clone> Dropdown<T> {
    pub fn current_value(&self) -> T {
        self.choices[self.current_idx].data.clone()
    }
    pub(crate) fn current_value_label(&self) -> &str {
        &self.choices[self.current_idx].label
    }
}

impl<T: 'static + Clone> Dropdown<T> {
    fn open_menu(&mut self, ctx: &mut EventCtx) {
        // TODO set current idx in menu
        let mut menu = Menu::new(
            ctx,
            self.choices
                .iter()
                .enumerate()
                .map(|(idx, c)| c.with_value(idx))
                .collect(),
        )
        .take_menu();
        let y1_below = self.btn.top_left.y + self.btn.dims.height + 15.0;

        menu.set_pos(ScreenPt::new(
            self.btn.top_left.x,
            // top_left_for_corner doesn't quite work
            if y1_below + menu.get_dims().height < ctx.canvas.window_height {
                y1_below
            } else {
                self.btn.top_left.y - 15.0 - menu.get_dims().height
            },
        ));
        self.menu = Some(menu);
    }
}

impl<T: 'static + Clone> WidgetImpl for Dropdown<T> {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }

    fn event(&mut self, ctx: &mut EventCtx, output: &mut WidgetOutput) {
        if let Some(ref mut m) = self.menu {
            let mut tmp_ouput = WidgetOutput::new();
            m.event(ctx, &mut tmp_ouput);
            if let Outcome::Clicked(_) = tmp_ouput.outcome {
                self.current_idx = self.menu.take().unwrap().take_current_choice();
                output.outcome = Outcome::Changed;
                let top_left = self.btn.top_left;
                self.btn = make_btn(
                    ctx,
                    &self.choices[self.current_idx].label,
                    &self.label,
                    self.is_persisten_split,
                );
                self.btn.set_pos(top_left);
                output.redo_layout = true;
            } else if ctx.normal_left_click() {
                if let Some(pt) = ctx.canvas.get_cursor_in_screen_space() {
                    if !ScreenRectangle::top_left(m.top_left, m.get_dims()).contains(pt) {
                        self.menu = None;
                    }
                } else {
                    self.menu = None;
                }
                if self.menu.is_some() {
                    ctx.input.unconsume_event();
                }
            }
        } else {
            self.btn.event(ctx, output);
            if let Outcome::Clicked(_) = output.outcome {
                output.outcome = Outcome::Nothing;
                self.open_menu(ctx);
            }
        }
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
        if let Some(ref m) = self.menu {
            // We need a background too! Add some padding and an outline.
            // TODO Little embedded Panel could make more sense?
            let pad = 5.0;
            let width = m.get_dims().width + 2.0 * pad;
            let height = m.get_dims().height + 2.0 * pad;
            let rect = Polygon::rounded_rectangle(width, height, Some(5.0));
            let draw_bg = g.upload(GeomBatch::from(vec![
                (Color::grey(0.3), rect.clone()),
                (
                    Color::WHITE,
                    rect.to_outline(Distance::meters(3.0)).unwrap(),
                ),
            ]));
            g.fork(
                Pt2D::new(0.0, 0.0),
                ScreenPt::new(m.top_left.x - pad, m.top_left.y - pad),
                1.0,
                Some(crate::drawing::MENU_Z),
            );
            g.redraw(&draw_bg);
            g.unfork();

            m.draw(g);

            // Dropdown menus often leak out of their Panel
            g.canvas
                .mark_covered_area(ScreenRectangle::top_left(m.top_left, m.get_dims()));
        }
    }

    fn can_restore(&self) -> bool {
        true
    }
    fn restore(&mut self, ctx: &mut EventCtx, prev: &Box<dyn WidgetImpl>) {
        let prev = prev.downcast_ref::<Dropdown<T>>().unwrap();
        if prev.menu.is_some() {
            self.open_menu(ctx);
            // TODO Preserve menu hovered item. Only matters if we've moved the cursor off the
            // menu.
        }
    }
}

fn make_btn(ctx: &EventCtx, label: &str, tooltip: &str, is_persisten_split: bool) -> Button {
    let mut builder = button_builder()
        .image_path("system/assets/tools/arrow_drop_down.svg")
        .image_dims(ScreenDims::square(12.0))
        .stack_spacing(16.0)
        .label_first();

    if is_persisten_split {
        // Quick hacks to make PersistentSplit's dropdown look a little better.
        // It's not ideal, but we only use one persistent split in the whole app
        // and it's front and center - we'll notice if something breaks.
        builder = builder
            .padding(EdgeInsets {
                top: 13.0,
                bottom: 13.0,
                left: 4.0,
                right: 4.0,
            })
            .bg_color(Color::CLEAR, ButtonState::Default)
            .outline(0.0, Color::CLEAR, ButtonState::Default);
    } else {
        builder = builder.label_text(label);
    }

    builder.build(ctx, tooltip)
}

// TODO: eventually this should be configurable.
// I'd like to base it on ColorScheme, but that currently lives in map_gui, so for now
// I've hardcoded the builder.
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
    let bg = Color::hex("#003046").alpha(0.8);
    let bg_hover = Color::hex("#003046");
    let bg_disabled = Color::grey(0.1);
    let outline = Color::hex("#003046").alpha(0.6);

    ButtonBuilder::new()
        .label_color(fg, ButtonState::Default)
        .label_color(fg_disabled, ButtonState::Disabled)
        .image_color(fg, ButtonState::Default)
        .image_color(fg_disabled, ButtonState::Disabled)
        .bg_color(bg, ButtonState::Default)
        .bg_color(bg_hover, ButtonState::Hover)
        .bg_color(bg_disabled, ButtonState::Disabled)
        .outline(2.0, outline, ButtonState::Default)
}

use geom::{CornerRadii, Distance, Polygon, Pt2D};

use crate::{
    Button, Choice, Color, ControlState, CornerRounding, EdgeInsets, EventCtx, GeomBatch, GfxCtx,
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
        let mut menu = Menu::new(
            ctx,
            self.choices
                .iter()
                .enumerate()
                .map(|(idx, c)| c.with_value(idx))
                .collect(),
        );
        menu.set_current(self.current_idx);
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
                output.outcome = Outcome::Changed(self.label.clone());
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

        if self.menu.is_some() {
            output.steal_focus(self.label.clone());
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
            let rect = Polygon::rounded_rectangle(width, height, 5.0);
            let draw_bg = g.upload(GeomBatch::from(vec![
                (g.style().field_bg, rect.clone()),
                (
                    g.style().dropdown_border,
                    rect.to_outline(Distance::meters(1.0)).unwrap(),
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
    fn restore(&mut self, ctx: &mut EventCtx, prev: &dyn WidgetImpl) {
        let prev = prev.downcast_ref::<Dropdown<T>>().unwrap();
        if prev.menu.is_some() {
            self.open_menu(ctx);
            // TODO Preserve menu hovered item. Only matters if we've moved the cursor off the
            // menu.
        }
    }
}

fn make_btn(ctx: &EventCtx, label: &str, tooltip: &str, is_persisten_split: bool) -> Button {
    // If we want to make Dropdown configurable, pass in or expose its button builder?
    let builder = if is_persisten_split {
        // Quick hacks to make PersistentSplit's dropdown look a little better.
        // It's not ideal, but we only use one persistent split in the whole app
        // and it's front and center - we'll notice if something breaks.
        ctx.style()
            .btn_solid
            .dropdown()
            .padding(EdgeInsets {
                top: 15.0,
                bottom: 15.0,
                left: 8.0,
                right: 8.0,
            })
            .corner_rounding(CornerRounding::CornerRadii(CornerRadii {
                top_left: 0.0,
                bottom_left: 0.0,
                bottom_right: 2.0,
                top_right: 2.0,
            }))
            // override any outline element within persistent split
            .outline((0.0, Color::CLEAR), ControlState::Default)
    } else {
        ctx.style().btn_outline.dropdown().label_text(label)
    };

    builder.build(ctx, tooltip)
}

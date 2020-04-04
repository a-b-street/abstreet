use crate::{
    Btn, Button, Choice, Color, EventCtx, GeomBatch, GfxCtx, InputResult, Menu, Outcome,
    ScreenDims, ScreenPt, ScreenRectangle, WidgetImpl,
};
use geom::{Distance, Polygon, Pt2D};

pub struct Dropdown<T: Clone> {
    current_idx: usize,
    btn: Button,
    // TODO Why not T?
    menu: Option<Menu<usize>>,
    label: String,
    blank_btn_label: bool,

    choices: Vec<Choice<T>>,
}

impl<T: 'static + PartialEq + Clone> Dropdown<T> {
    pub fn new(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        choices: Vec<Choice<T>>,
        // TODO Ideally builder style
        blank_btn_label: bool,
    ) -> Dropdown<T> {
        let current_idx = choices
            .iter()
            .position(|c| c.data == default_value)
            .unwrap();

        Dropdown {
            current_idx,
            btn: make_btn(ctx, &choices[current_idx].label, label, blank_btn_label),
            menu: None,
            label: label.to_string(),
            blank_btn_label,
            choices,
        }
    }

    pub fn current_value(&self) -> T {
        self.choices[self.current_idx].data.clone()
    }
    pub(crate) fn current_value_label(&self) -> String {
        self.choices[self.current_idx].label.clone()
    }
}

impl<T: 'static + Clone> WidgetImpl for Dropdown<T> {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }

    fn event(&mut self, ctx: &mut EventCtx, redo_layout: &mut bool) -> Option<Outcome> {
        if let Some(ref mut m) = self.menu {
            m.event(ctx, redo_layout);
            match m.state {
                InputResult::StillActive => {}
                InputResult::Canceled => {
                    self.menu = None;
                }
                InputResult::Done(_, idx) => {
                    self.menu = None;
                    self.current_idx = idx;
                    let top_left = self.btn.top_left;
                    self.btn = make_btn(
                        ctx,
                        &self.choices[self.current_idx].label,
                        &self.label,
                        self.blank_btn_label,
                    );
                    self.btn.set_pos(top_left);
                    *redo_layout = true;
                }
            }
        } else {
            if self.btn.event(ctx, redo_layout).is_some() {
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

        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
        if let Some(ref m) = self.menu {
            // We need a background too! Add some padding and an outline.
            // TODO Little embedded Composite could make more sense?
            let pad = 5.0;
            let width = m.get_dims().width + 2.0 * pad;
            let height = m.get_dims().height + 2.0 * pad;
            let rect = Polygon::rounded_rectangle(width, height, Some(5.0));
            let draw_bg = g.upload(GeomBatch::from(vec![
                (Color::grey(0.3), rect.clone()),
                (Color::WHITE, rect.to_outline(Distance::meters(3.0))),
            ]));
            g.fork(
                Pt2D::new(0.0, 0.0),
                ScreenPt::new(m.top_left.x - pad, m.top_left.y - pad),
                1.0,
                // Between SCREENSPACE_Z and TOOLTIP_Z
                Some(0.1),
            );
            g.redraw(&draw_bg);
            g.unfork();

            m.draw(g);

            // Dropdown menus often leak out of their Composite
            g.canvas
                .mark_covered_area(ScreenRectangle::top_left(m.top_left, m.get_dims()));
        }
    }
}

fn make_btn(ctx: &EventCtx, name: &str, label: &str, blank_btn_label: bool) -> Button {
    (if blank_btn_label {
        Btn::text_fg("▼")
    } else {
        Btn::text_fg(format!("{} ▼", name))
    })
    .build(ctx, label, None)
    .take_btn()
}

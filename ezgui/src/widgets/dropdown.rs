use crate::{
    Btn, Button, Choice, Color, EventCtx, GfxCtx, InputResult, Menu, Outcome, ScreenDims, ScreenPt,
    WidgetImpl,
};
use geom::{Polygon, Pt2D};

pub struct Dropdown<T: Clone> {
    current_idx: usize,
    btn: Button,
    // TODO Why not T?
    menu: Option<Menu<usize>>,
    label: String,

    choices: Vec<Choice<T>>,
}

impl<T: 'static + PartialEq + Clone> Dropdown<T> {
    pub fn new(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        choices: Vec<Choice<T>>,
    ) -> Dropdown<T> {
        let current_idx = choices
            .iter()
            .position(|c| c.data == default_value)
            .unwrap();

        Dropdown {
            current_idx,
            btn: make_btn(ctx, &choices[current_idx].label, label),
            menu: None,
            label: label.to_string(),
            choices,
        }
    }

    pub fn current_value(&self) -> T {
        self.choices[self.current_idx].data.clone()
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
                    // TODO Recalculate widgets when this happens... outline around button should
                    // change
                    self.btn = make_btn(ctx, &self.choices[self.current_idx].label, &self.label);
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
                menu.set_pos(ScreenPt::new(
                    self.btn.top_left.x,
                    self.btn.top_left.y + self.btn.dims.height + 15.0,
                ));
                self.menu = Some(menu);
            }
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.btn.draw(g);
        if let Some(ref m) = self.menu {
            // We need a background too!
            g.fork(Pt2D::new(0.0, 0.0), m.top_left, 1.0, Some(0.1));
            g.draw_polygon(
                Color::grey(0.3),
                &Polygon::rounded_rectangle(m.get_dims().width, m.get_dims().height, 5.0),
            );
            g.unfork();

            m.draw(g);
        }
    }
}

fn make_btn(ctx: &EventCtx, name: &str, label: &str) -> Button {
    Btn::text_fg(format!("{} ▼", name))
        .build(ctx, label, None)
        .take_btn()
}

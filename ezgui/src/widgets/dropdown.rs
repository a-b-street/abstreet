use crate::widgets::Widget;
use crate::{
    Btn, Button, Choice, Color, EventCtx, GfxCtx, InputResult, PopupMenu, ScreenDims, ScreenPt,
    ScreenRectangle,
};
use geom::{Polygon, Pt2D};
use std::any::Any;

pub struct Dropdown {
    current_idx: usize,
    btn: Button,
    menu: Option<PopupMenu<usize>>,
    label: String,

    choices: Vec<Choice<Box<dyn Any>>>,
}

impl Dropdown {
    pub fn new<T: 'static + PartialEq>(
        ctx: &EventCtx,
        label: &str,
        default_value: T,
        choices: Vec<Choice<T>>,
    ) -> Dropdown {
        let current_idx = choices
            .iter()
            .position(|c| c.data == default_value)
            .unwrap();

        Dropdown {
            current_idx,
            btn: make_btn(ctx, &choices[current_idx].label, label),
            menu: None,
            label: label.to_string(),

            choices: choices
                .into_iter()
                .map(|c| {
                    // TODO Can't use with_value here :(
                    let data: Box<dyn Any> = Box::new(c.data);
                    Choice {
                        label: c.label,
                        data,
                        hotkey: c.hotkey,
                        active: c.active,
                        tooltip: c.tooltip,
                    }
                })
                .collect(),
        }
    }

    // If true, widgets should be recomputed.
    pub fn event(&mut self, ctx: &mut EventCtx, our_rect: &ScreenRectangle) -> bool {
        if let Some(ref mut m) = self.menu {
            m.event(ctx);
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
                    return true;
                }
            }
        } else {
            self.btn.event(ctx);
            if self.btn.clicked() {
                // TODO set current idx in menu
                // TODO Choice::map_value?
                let mut menu = PopupMenu::new(
                    ctx,
                    self.choices
                        .iter()
                        .enumerate()
                        .map(|(idx, c)| c.with_value(idx))
                        .collect(),
                );
                menu.set_pos(ScreenPt::new(our_rect.x1, our_rect.y2 + 15.0));
                self.menu = Some(menu);
            }
        }

        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
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

    // TODO This invalidates the entire widget!
    pub fn take_value<T: 'static>(&mut self) -> T {
        let data: Box<dyn Any> = self.choices.remove(self.current_idx).data;
        let boxed: Box<T> = data.downcast().unwrap();
        *boxed
    }
}

impl Widget for Dropdown {
    fn get_dims(&self) -> ScreenDims {
        self.btn.get_dims()
    }

    fn set_pos(&mut self, top_left: ScreenPt) {
        self.btn.set_pos(top_left);
    }
}

fn make_btn(ctx: &EventCtx, name: &str, label: &str) -> Button {
    Btn::text_fg(format!("{} ▼", name))
        .build(ctx, label, None)
        .take_btn()
}

use std::cell::RefCell;
use std::rc::Rc;

use geom::{Polygon, Pt2D};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text,
    VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

// TODO Lift to widgetry
pub struct RectangularSelector {
    panel: Panel,
    region: Rc<RefCell<Option<Polygon>>>,
    corners: Option<(Pt2D, Pt2D, bool)>,
}

impl RectangularSelector {
    pub fn new_state(
        ctx: &mut EventCtx,
        region: Rc<RefCell<Option<Polygon>>>,
    ) -> Box<dyn State<App>> {
        Box::new(RectangularSelector {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Select a rectangular region")
                        .small_heading()
                        .into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Text::from_all(vec![
                    Line("Hold "),
                    Line(Key::LeftControl.describe()).fg(ctx.style().text_hotkey_color),
                    Line(", then click and drag to draw"),
                ])
                .into_widget(ctx),
                Widget::row(vec![
                    ctx.style()
                        .btn_solid_primary
                        .text("Apply")
                        .hotkey(Key::Enter)
                        .build_def(ctx),
                    ctx.style()
                        .btn_solid_destructive
                        .text("Clear")
                        .build_def(ctx),
                ]),
            ]))
            .aligned(HorizontalAlignment::Right, VerticalAlignment::Top)
            .build(ctx),
            region,
            corners: None,
        })
    }
}

impl State<App> for RectangularSelector {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        if ctx.is_key_down(Key::LeftControl) {
            if ctx.input.left_mouse_button_released() {
                if let Some((_, _, ref mut dragging)) = self.corners {
                    *dragging = false;
                }
            }
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if ctx.input.left_mouse_button_pressed() {
                    self.corners = Some((pt, pt, true));
                }
                if let Some((_, ref mut pt2, dragging)) = self.corners {
                    if dragging {
                        *pt2 = pt;
                    }
                }
            }
        } else {
            ctx.canvas_movement();
        }

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "Clear" => {
                    self.region.replace(None);
                    return Transition::Pop;
                }
                "Apply" => {
                    if let Some(rect) = self
                        .corners
                        .and_then(|(pt1, pt2, _)| Polygon::rectangle_two_corners(pt1, pt2))
                    {
                        self.region.replace(Some(rect));
                    }
                    return Transition::Pop;
                }
                _ => unreachable!(),
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        if let Some(p) = self.region.borrow().clone() {
            g.draw_polygon(Color::BLUE.alpha(0.5), p);
        }
        if let Some((pt1, pt2, _)) = self.corners {
            if let Some(p) = Polygon::rectangle_two_corners(pt1, pt2) {
                g.draw_polygon(Color::RED.alpha(0.5), p);
            }
        }
    }
}

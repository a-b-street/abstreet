use geom::{Polygon, Pt2D, Triangle};
use map_gui::tools::PopupMsg;
use widgetry::{
    EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text,
    TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};

pub struct PolygonDebugger {
    panel: Panel,
    noun: String,
    items: Vec<Item>,
    idx: usize,
    center: Option<Pt2D>,
}

pub enum Item {
    Point(Pt2D),
    Triangle(Triangle),
    Polygon(Polygon),
}

impl PolygonDebugger {
    pub fn new_state(
        ctx: &mut EventCtx,
        noun: &str,
        items: Vec<Item>,
        center: Option<Pt2D>,
    ) -> Box<dyn State<App>> {
        if items.is_empty() {
            return PopupMsg::new_state(ctx, "Woops", vec![format!("No {}, never mind", noun)]);
        }

        Box::new(PolygonDebugger {
            panel: Panel::new_builder(Widget::col(vec![
                Widget::row(vec![
                    Line("Geometry debugger").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                Widget::row(vec![
                    // TODO Make these inactive when on the first/last. More generally, refactor
                    // some kind of pagination helper.
                    ctx.style()
                        .btn_prev()
                        .hotkey(Key::LeftArrow)
                        .build_widget(ctx, "previous"),
                    "noun X/Y".text_widget(ctx).named("pointer"),
                    ctx.style()
                        .btn_next()
                        .hotkey(Key::RightArrow)
                        .build_widget(ctx, "next"),
                ])
                .evenly_spaced(),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            noun: noun.to_string(),
            items,
            idx: 0,
            center,
        })
    }
}

impl State<App> for PolygonDebugger {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Outcome::Clicked(x) = self.panel.event(ctx) {
                match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "previous" => {
                    if self.idx != 0 {
                        self.idx -= 1;
                    }
                }
                "next" => {
                    if self.idx != self.items.len() - 1 {
                        self.idx += 1;
                    }
                }
                _ => unreachable!(),
            }
        }
        self.panel.replace(
            ctx,
            "pointer",
            format!("{} {}/{}", self.noun, self.idx + 1, self.items.len()).text_widget(ctx),
        );

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // This is drawn in screen-space, so zooming doesn't affect the text size
        let mut batch = GeomBatch::new();

        match &self.items[self.idx] {
            Item::Point(pt) => {
                batch.append(
                    Text::from(self.idx.to_string())
                        .bg(app.cs.panel_bg)
                        .render(g)
                        .centered_on(g.canvas.map_to_screen(*pt).to_pt()),
                );
            }
            Item::Triangle(ref tri) => {
                for pt in &[tri.pt1, tri.pt2, tri.pt3] {
                    batch.append(
                        Text::from(self.idx.to_string())
                            .bg(app.cs.panel_bg)
                            .render(g)
                            .centered_on(g.canvas.map_to_screen(*pt).to_pt()),
                    );
                }
                g.draw_polygon(app.cs.selected, Polygon::from_triangle(tri));
            }
            Item::Polygon(ref poly) => {
                g.draw_polygon(app.cs.selected, poly.clone());
                batch.append(
                    Text::from(self.idx.to_string())
                        .bg(app.cs.panel_bg)
                        .render(g)
                        .centered_on(g.canvas.map_to_screen(poly.center()).to_pt()),
                );
            }
        }
        if let Some(pt) = self.center {
            batch.append(
                Text::from("c")
                    .bg(app.cs.panel_bg)
                    .render(g)
                    .centered_on(g.canvas.map_to_screen(pt).to_pt()),
            );
        }

        let draw = g.upload(batch);
        g.fork_screenspace();
        g.redraw(&draw);
        g.unfork();

        self.panel.draw(g);
    }
}

use crate::app::App;
use crate::colors;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Slider, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Polygon, Pt2D, Triangle};

pub struct PolygonDebugger {
    composite: Composite,
    noun: String,
    items: Vec<Item>,
    center: Option<Pt2D>,
}

pub enum Item {
    Point(Pt2D),
    Triangle(Triangle),
    Polygon(Polygon),
}

impl PolygonDebugger {
    pub fn new(
        ctx: &mut EventCtx,
        noun: &str,
        items: Vec<Item>,
        center: Option<Pt2D>,
    ) -> Box<dyn State> {
        Box::new(PolygonDebugger {
            composite: make_panel(ctx),
            noun: noun.to_string(),
            items,
            center,
        })
    }
}

impl State for PolygonDebugger {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        ctx.canvas_movement();

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "previous" => {
                    let idx = (self.composite.slider("slider").get_percent()
                        * (self.items.len() - 1) as f64) as usize;
                    if idx != 0 {
                        self.composite
                            .slider_mut("slider")
                            .set_percent(ctx, (idx - 1) as f64 / (self.items.len() - 1) as f64);
                    }
                }
                "next" => {
                    let idx = (self.composite.slider("slider").get_percent()
                        * (self.items.len() - 1) as f64) as usize;
                    if idx != self.items.len() - 1 {
                        self.composite
                            .slider_mut("slider")
                            .set_percent(ctx, (idx + 1) as f64 / (self.items.len() - 1) as f64);
                    }
                }
                _ => unreachable!(),
            },
            None => {}
        }
        // TODO Could be more efficient here
        let idx = (self.composite.slider("slider").get_percent() * (self.items.len() - 1) as f64)
            as usize;
        self.composite.replace(
            ctx,
            "pointer",
            format!("{} {}/{}", self.noun, idx + 1, self.items.len())
                .draw_text(ctx)
                .named("pointer"),
        );

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // This is drawn in screen-space, so zooming doesn't affect the text size
        let mut batch = GeomBatch::new();

        let idx = (self.composite.slider("slider").get_percent() * (self.items.len() - 1) as f64)
            as usize;
        match &self.items[idx] {
            Item::Point(pt) => {
                batch.add_centered(
                    Text::from(Line(idx.to_string()))
                        .bg(colors::PANEL_BG)
                        .render_g(g),
                    g.canvas.map_to_screen(*pt).to_pt(),
                );
            }
            Item::Triangle(ref tri) => {
                for pt in &[tri.pt1, tri.pt2, tri.pt3] {
                    batch.add_centered(
                        Text::from(Line(idx.to_string()))
                            .bg(colors::PANEL_BG)
                            .render_g(g),
                        g.canvas.map_to_screen(*pt).to_pt(),
                    );
                }
                g.draw_polygon(app.cs.get("selected"), &Polygon::from_triangle(tri));
            }
            Item::Polygon(ref poly) => {
                g.draw_polygon(app.cs.get("selected"), poly);
                batch.add_centered(
                    Text::from(Line(idx.to_string()))
                        .bg(colors::PANEL_BG)
                        .render_g(g),
                    g.canvas.map_to_screen(poly.center()).to_pt(),
                );
            }
        }
        if let Some(pt) = self.center {
            batch.add_centered(
                Text::from(Line("c")).bg(colors::PANEL_BG).render_g(g),
                g.canvas.map_to_screen(pt).to_pt(),
            );
        }

        let draw = g.upload(batch);
        g.fork_screenspace();
        g.redraw(&draw);
        g.unfork();

        self.composite.draw(g);
    }
}

fn make_panel(ctx: &mut EventCtx) -> Composite {
    Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Line("Geometry debugger")
                    .small_heading()
                    .draw(ctx)
                    .margin(5),
                Btn::text_fg("X")
                    .build(ctx, "close", hotkey(Key::Escape))
                    .align_right(),
            ]),
            Widget::row(vec![
                // TODO inactive
                Btn::text_fg("<").build(ctx, "previous", hotkey(Key::LeftArrow)),
                "noun X/Y".draw_text(ctx).named("pointer"),
                Btn::text_fg(">").build(ctx, "next", hotkey(Key::RightArrow)),
            ])
            .evenly_spaced(),
            Slider::horizontal(ctx, 100.0, 25.0, 0.0)
                .named("slider")
                .centered_horiz(),
        ])
        .bg(colors::PANEL_BG)
        .padding(5),
    )
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

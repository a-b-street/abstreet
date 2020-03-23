use crate::app::App;
use crate::colors;
use crate::game::{State, Transition};
use crate::helpers::ID;
use crate::render::calculate_corners;
use abstutil::Timer;
use ezgui::{
    hotkey, Btn, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Slider,
    Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Polygon, Pt2D, Triangle};

pub struct PolygonDebugger {
    composite: Composite,
    noun: String,
    items: Vec<Item>,
    center: Option<Pt2D>,
}

enum Item {
    Point(Pt2D),
    Triangle(Triangle),
    Polygon(Polygon),
}

impl PolygonDebugger {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Option<PolygonDebugger> {
        match app.primary.current_selection {
            Some(ID::Intersection(id)) => {
                let i = app.primary.map.get_i(id);
                if app
                    .per_obj
                    .action(ctx, Key::X, "debug intersection geometry")
                {
                    let pts = i.polygon.points();
                    let mut pts_without_last = pts.clone();
                    pts_without_last.pop();
                    return Some(PolygonDebugger {
                        composite: make_panel(ctx),
                        noun: "point".to_string(),
                        items: pts.iter().map(|pt| Item::Point(*pt)).collect(),
                        center: Some(Pt2D::center(&pts_without_last)),
                    });
                } else if app.per_obj.action(ctx, Key::F2, "debug sidewalk corners") {
                    return Some(PolygonDebugger {
                        composite: make_panel(ctx),
                        noun: "corner".to_string(),
                        items: calculate_corners(
                            i,
                            &app.primary.map,
                            &mut Timer::new("calculate corners"),
                        )
                        .into_iter()
                        .map(|poly| Item::Polygon(poly))
                        .collect(),
                        center: None,
                    });
                }
            }
            Some(ID::Lane(id)) => {
                if app.per_obj.action(ctx, Key::X, "debug lane geometry") {
                    return Some(PolygonDebugger {
                        composite: make_panel(ctx),
                        noun: "point".to_string(),
                        items: app
                            .primary
                            .map
                            .get_l(id)
                            .lane_center_pts
                            .points()
                            .iter()
                            .map(|pt| Item::Point(*pt))
                            .collect(),
                        center: None,
                    });
                } else if app
                    .per_obj
                    .action(ctx, Key::F2, "debug lane triangles geometry")
                {
                    return Some(PolygonDebugger {
                        composite: make_panel(ctx),
                        noun: "triangle".to_string(),
                        items: app
                            .primary
                            .draw_map
                            .get_l(id)
                            .polygon
                            .triangles()
                            .into_iter()
                            .map(|tri| Item::Triangle(tri))
                            .collect(),
                        center: None,
                    });
                }
            }
            Some(ID::Area(id)) => {
                if app.per_obj.action(ctx, Key::X, "debug area geometry") {
                    let pts = &app.primary.map.get_a(id).polygon.points();
                    let center = if pts[0] == *pts.last().unwrap() {
                        // TODO The center looks really wrong for Volunteer Park and others, but I
                        // think it's because they have many points along some edges.
                        Pt2D::center(&pts.iter().skip(1).cloned().collect())
                    } else {
                        Pt2D::center(pts)
                    };
                    return Some(PolygonDebugger {
                        composite: make_panel(ctx),
                        noun: "point".to_string(),
                        items: pts.iter().map(|pt| Item::Point(*pt)).collect(),
                        center: Some(center),
                    });
                } else if app.per_obj.action(ctx, Key::F2, "debug area triangles") {
                    return Some(PolygonDebugger {
                        composite: make_panel(ctx),
                        noun: "triangle".to_string(),
                        items: app
                            .primary
                            .map
                            .get_a(id)
                            .polygon
                            .triangles()
                            .into_iter()
                            .map(|tri| Item::Triangle(tri))
                            .collect(),
                        center: None,
                    });
                }
            }
            _ => {}
        }
        None
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
        let idx = (self.composite.slider("slider").get_percent() * (self.items.len() - 1) as f64)
            as usize;
        match &self.items[idx] {
            Item::Point(pt) => {
                g.draw_text_at(Text::from(Line(idx.to_string())).with_bg(), *pt);
            }
            Item::Triangle(ref tri) => {
                for pt in &[tri.pt1, tri.pt2, tri.pt3] {
                    g.draw_text_at(Text::from(Line(idx.to_string())).with_bg(), *pt);
                }
                g.draw_polygon(app.cs.get("selected"), &Polygon::from_triangle(tri));
            }
            Item::Polygon(ref poly) => {
                g.draw_polygon(app.cs.get("selected"), poly);
                g.draw_text_at(Text::from(Line(idx.to_string())).with_bg(), poly.center());
            }
        }
        if let Some(pt) = self.center {
            g.draw_text_at(Text::from(Line("c")).with_bg(), pt);
        }

        self.composite.draw(g);
    }
}

fn make_panel(ctx: &mut EventCtx) -> Composite {
    Composite::new(
        Widget::col(vec![
            Widget::row(vec![
                Line("Geometry debugger").roboto_bold().draw(ctx).margin(5),
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

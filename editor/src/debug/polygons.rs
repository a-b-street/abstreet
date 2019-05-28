use crate::helpers::ID;
use crate::render::calculate_corners;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{EventCtx, GfxCtx, Key, ModalMenu, Slider, Text};
use geom::{Polygon, Pt2D, Triangle};

pub struct PolygonDebugger {
    menu: ModalMenu,
    items: Vec<Item>,
    slider: Slider,
    center: Option<Pt2D>,
}

enum Item {
    Point(Pt2D),
    Triangle(Triangle),
    Polygon(Polygon),
}

impl PolygonDebugger {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Option<PolygonDebugger> {
        let menu = ModalMenu::new(
            "Polygon Debugger",
            vec![
                (Some(Key::Escape), "quit"),
                (Some(Key::Dot), "next item"),
                (Some(Key::Comma), "prev item"),
                (Some(Key::F), "first item"),
                (Some(Key::L), "last item"),
            ],
            ctx,
        );
        match ui.primary.current_selection {
            Some(ID::Intersection(id)) => {
                let i = ui.primary.map.get_i(id);
                if ctx
                    .input
                    .contextual_action(Key::X, "debug intersection geometry")
                {
                    let pts = i.polygon.points();
                    let mut pts_without_last = pts.clone();
                    pts_without_last.pop();
                    return Some(PolygonDebugger {
                        menu,
                        items: pts.iter().map(|pt| Item::Point(*pt)).collect(),
                        slider: Slider::new(),
                        center: Some(Pt2D::center(&pts_without_last)),
                    });
                } else if ctx
                    .input
                    .contextual_action(Key::F2, "debug sidewalk corners")
                {
                    let items: Vec<Item> =
                        calculate_corners(i, &ui.primary.map, &mut Timer::new("calculate corners"))
                            .into_iter()
                            .map(Item::Polygon)
                            .collect();
                    return Some(PolygonDebugger {
                        menu,
                        slider: Slider::new(),
                        items,
                        center: None,
                    });
                }
            }
            Some(ID::Lane(id)) => {
                if ctx.input.contextual_action(Key::X, "debug lane geometry") {
                    let items: Vec<Item> = ui
                        .primary
                        .map
                        .get_l(id)
                        .lane_center_pts
                        .points()
                        .iter()
                        .map(|pt| Item::Point(*pt))
                        .collect();
                    return Some(PolygonDebugger {
                        menu,
                        slider: Slider::new(),
                        items,
                        center: None,
                    });
                } else if ctx.input.contextual_action(Key::F2, "debug lane triangles") {
                    let items: Vec<Item> = ui
                        .primary
                        .draw_map
                        .get_l(id)
                        .polygon
                        .triangles()
                        .into_iter()
                        .map(Item::Triangle)
                        .collect();
                    return Some(PolygonDebugger {
                        menu,
                        slider: Slider::new(),
                        items,
                        center: None,
                    });
                }
            }
            Some(ID::Area(id)) => {
                if ctx.input.contextual_action(Key::X, "debug area geometry") {
                    let pts = &ui.primary.map.get_a(id).polygon.points();
                    let center = if pts[0] == *pts.last().unwrap() {
                        // TODO The center looks really wrong for Volunteer Park and others, but I
                        // think it's because they have many points along some edges.
                        Pt2D::center(&pts.iter().skip(1).cloned().collect())
                    } else {
                        Pt2D::center(pts)
                    };
                    return Some(PolygonDebugger {
                        menu,
                        items: pts.iter().map(|pt| Item::Point(*pt)).collect(),
                        slider: Slider::new(),
                        center: Some(center),
                    });
                } else if ctx.input.contextual_action(Key::F2, "debug area triangles") {
                    let items: Vec<Item> = ui
                        .primary
                        .map
                        .get_a(id)
                        .polygon
                        .triangles()
                        .into_iter()
                        .map(Item::Triangle)
                        .collect();
                    return Some(PolygonDebugger {
                        menu,
                        slider: Slider::new(),
                        items,
                        center: None,
                    });
                }
            }
            _ => {}
        }
        None
    }

    // True when done
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        let current = self.slider.get_value(self.items.len());

        let mut txt = Text::prompt("Polygon Debugger");
        txt.add_line(format!("Item {}/{}", current + 1, self.items.len()));
        self.menu.handle_event(ctx, Some(txt));
        ctx.canvas.handle_event(ctx.input);

        if self.menu.action("quit") {
            return true;
        } else if current != self.items.len() - 1 && self.menu.action("next item") {
            self.slider.set_value(ctx, current + 1, self.items.len());
        } else if current != self.items.len() - 1 && self.menu.action("last item") {
            self.slider.set_percent(ctx, 1.0);
        } else if current != 0 && self.menu.action("prev item") {
            self.slider.set_value(ctx, current - 1, self.items.len());
        } else if current != 0 && self.menu.action("first item") {
            self.slider.set_percent(ctx, 0.0);
        }

        self.slider.event(ctx);

        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let current = self.slider.get_value(self.items.len());

        match self.items[current] {
            Item::Point(pt) => {
                g.draw_text_at(&Text::from_line(format!("{}", current)), pt);
            }
            Item::Triangle(ref tri) => {
                for pt in &[tri.pt1, tri.pt2, tri.pt3] {
                    g.draw_text_at(&Text::from_line(format!("{}", current)), *pt);
                }
                g.draw_polygon(ui.cs.get("selected"), &Polygon::from_triangle(tri));
            }
            Item::Polygon(ref poly) => {
                g.draw_polygon(ui.cs.get("selected"), poly);
                g.draw_text_at(&Text::from_line(format!("{}", current)), poly.center());
            }
        }
        if let Some(pt) = self.center {
            g.draw_text_at(&Text::from_line("c".to_string()), pt);
        }
        self.menu.draw(g);
        self.slider.draw(g);
    }
}

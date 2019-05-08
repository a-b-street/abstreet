use crate::helpers::ID;
use crate::render::calculate_corners;
use crate::ui::UI;
use abstutil::Timer;
use ezgui::{EventCtx, GfxCtx, Key, ModalMenu, Text};
use geom::{Polygon, Pt2D, Triangle};

pub struct PolygonDebugger {
    menu: ModalMenu,
    items: Vec<Item>,
    current: usize,
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
                        current: 0,
                        center: Some(Pt2D::center(&pts_without_last)),
                    });
                } else if ctx
                    .input
                    .contextual_action(Key::F2, "debug sidewalk corners")
                {
                    return Some(PolygonDebugger {
                        menu,
                        items: calculate_corners(
                            i,
                            &ui.primary.map,
                            &mut Timer::new("calculate corners"),
                        )
                        .into_iter()
                        .map(Item::Polygon)
                        .collect(),
                        current: 0,
                        center: None,
                    });
                }
            }
            Some(ID::Lane(id)) => {
                if ctx.input.contextual_action(Key::X, "debug lane geometry") {
                    return Some(PolygonDebugger {
                        menu,
                        items: ui
                            .primary
                            .map
                            .get_l(id)
                            .lane_center_pts
                            .points()
                            .iter()
                            .map(|pt| Item::Point(*pt))
                            .collect(),
                        current: 0,
                        center: None,
                    });
                } else if ctx.input.contextual_action(Key::F2, "debug lane triangles") {
                    return Some(PolygonDebugger {
                        menu,
                        items: ui
                            .primary
                            .draw_map
                            .get_l(id)
                            .polygon
                            .triangles()
                            .into_iter()
                            .map(Item::Triangle)
                            .collect(),
                        current: 0,
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
                        current: 0,
                        center: Some(center),
                    });
                } else if ctx.input.contextual_action(Key::F2, "debug area triangles") {
                    return Some(PolygonDebugger {
                        menu,
                        items: ui
                            .primary
                            .map
                            .get_a(id)
                            .polygon
                            .triangles()
                            .into_iter()
                            .map(Item::Triangle)
                            .collect(),
                        current: 0,
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
        self.menu.handle_event(ctx, None);
        ctx.canvas.handle_event(ctx.input);
        if self.menu.action("quit") {
            return true;
        } else if self.current != self.items.len() - 1 && self.menu.action("next item") {
            self.current += 1;
        } else if self.current != self.items.len() - 1 && self.menu.action("last item") {
            self.current = self.items.len() - 1;
        } else if self.current != 0 && self.menu.action("prev item") {
            self.current -= 1;
        } else if self.current != 0 && self.menu.action("first item") {
            self.current = 0;
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        match self.items[self.current] {
            Item::Point(pt) => {
                g.draw_text_at(&Text::from_line(format!("{}", self.current)), pt);
            }
            Item::Triangle(ref tri) => {
                for pt in &[tri.pt1, tri.pt2, tri.pt3] {
                    g.draw_text_at(&Text::from_line(format!("{}", self.current)), *pt);
                }
                g.draw_polygon(ui.cs.get("selected"), &Polygon::from_triangle(tri));
            }
            Item::Polygon(ref poly) => {
                g.draw_polygon(ui.cs.get("selected"), poly);
                g.draw_text_at(&Text::from_line(format!("{}", self.current)), poly.center());
            }
        }
        if let Some(pt) = self.center {
            g.draw_text_at(&Text::from_line("c".to_string()), pt);
        }
        self.menu.draw(g);
    }
}

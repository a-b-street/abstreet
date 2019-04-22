use crate::objects::{DrawCtx, ID};
use crate::plugins::{BlockingPlugin, PluginCtx};
use crate::render::calculate_corners;
use abstutil::Timer;
use ezgui::{GfxCtx, Key, Text};
use geom::{Polygon, Pt2D, Triangle};

enum Item {
    Point(Pt2D),
    Triangle(Triangle),
    Polygon(Polygon),
}

pub struct DebugPolygon {
    items: Vec<Item>,
    current: usize,
    center: Option<Pt2D>,
}

impl DebugPolygon {
    pub fn new(ctx: &mut PluginCtx) -> Option<DebugPolygon> {
        match ctx.primary.current_selection {
            Some(ID::Intersection(id)) => {
                let i = ctx.primary.map.get_i(id);
                if ctx
                    .input
                    .contextual_action(Key::X, "debug intersection geometry")
                {
                    let pts = i.polygon.points();
                    let mut pts_without_last = pts.clone();
                    pts_without_last.pop();
                    return Some(DebugPolygon {
                        items: pts.iter().map(|pt| Item::Point(*pt)).collect(),
                        current: 0,
                        center: Some(Pt2D::center(&pts_without_last)),
                    });
                } else if ctx
                    .input
                    .contextual_action(Key::F2, "debug sidewalk corners")
                {
                    return Some(DebugPolygon {
                        items: calculate_corners(
                            i,
                            &ctx.primary.map,
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
                    return Some(DebugPolygon {
                        items: ctx
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
                    return Some(DebugPolygon {
                        items: ctx
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
                    let pts = &ctx.primary.map.get_a(id).polygon.points();
                    let center = if pts[0] == *pts.last().unwrap() {
                        // TODO The center looks really wrong for Volunteer Park and others, but I
                        // think it's because they have many points along some edges.
                        Pt2D::center(&pts.iter().skip(1).cloned().collect())
                    } else {
                        Pt2D::center(pts)
                    };
                    return Some(DebugPolygon {
                        items: pts.iter().map(|pt| Item::Point(*pt)).collect(),
                        current: 0,
                        center: Some(center),
                    });
                } else if ctx.input.contextual_action(Key::F2, "debug area triangles") {
                    return Some(DebugPolygon {
                        items: ctx
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
}

impl BlockingPlugin for DebugPolygon {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Polygon Debugger", &ctx.canvas);
        if ctx.input.modal_action("quit") {
            return false;
        } else if self.current != self.items.len() - 1 && ctx.input.modal_action("next item") {
            self.current += 1;
        } else if self.current != self.items.len() - 1 && ctx.input.modal_action("last item") {
            self.current = self.items.len() - 1;
        } else if self.current != 0 && ctx.input.modal_action("prev item") {
            self.current -= 1;
        } else if self.current != 0 && ctx.input.modal_action("first item") {
            self.current = 0;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        match self.items[self.current] {
            Item::Point(pt) => {
                g.draw_text_at(&Text::from_line(format!("{}", self.current)), pt);
            }
            Item::Triangle(ref tri) => {
                for pt in &[tri.pt1, tri.pt2, tri.pt3] {
                    g.draw_text_at(&Text::from_line(format!("{}", self.current)), *pt);
                }
                g.draw_polygon(ctx.cs.get("selected"), &Polygon::from_triangle(tri));
            }
            Item::Polygon(ref poly) => {
                g.draw_polygon(ctx.cs.get("selected"), poly);
                g.draw_text_at(&Text::from_line(format!("{}", self.current)), poly.center());
            }
        }
        if let Some(pt) = self.center {
            g.draw_text_at(&Text::from_line("c".to_string()), pt);
        }
    }
}

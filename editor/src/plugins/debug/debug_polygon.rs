use crate::objects::{Ctx, ID};
use crate::plugins::{Plugin, PluginCtx};
use ezgui::{GfxCtx, Key, Text};
use geom::{Pt2D, Triangle};

enum Item {
    Point(Pt2D),
    Triangle(Triangle),
}

pub struct DebugPolygon {
    items: Vec<Item>,
    current: usize,
}

impl DebugPolygon {
    pub fn new(ctx: &mut PluginCtx) -> Option<DebugPolygon> {
        match ctx.primary.current_selection {
            Some(ID::Intersection(id)) => {
                if ctx
                    .input
                    .contextual_action(Key::X, "debug intersection geometry")
                {
                    return Some(DebugPolygon {
                        items: ctx
                            .primary
                            .map
                            .get_i(id)
                            .polygon
                            .points()
                            .iter()
                            .map(|pt| Item::Point(*pt))
                            .collect(),
                        current: 0,
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
                    });
                }
            }
            _ => {}
        }
        None
    }
}

impl Plugin for DebugPolygon {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        ctx.input.set_mode("Polygon Debugger", &ctx.canvas);
        if ctx.input.modal_action("quit") {
            return false;
        } else if self.current != self.items.len() - 1 && ctx.input.modal_action("next item") {
            self.current += 1;
        } else if self.current != 0 && ctx.input.modal_action("prev item") {
            self.current -= 1;
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, _ctx: &Ctx) {
        match self.items[self.current] {
            Item::Point(pt) => {
                g.draw_text_at(Text::from_line(format!("{}", self.current)), pt);
            }
            Item::Triangle(ref tri) => {
                for pt in &[tri.pt1, tri.pt2, tri.pt3] {
                    g.draw_text_at(Text::from_line(format!("{}", self.current)), *pt);
                }
            }
        }
    }
}

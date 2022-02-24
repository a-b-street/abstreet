use geom::{Distance, PolyLine, Polygon, Pt2D, Ring};

use crate::{Color, EventCtx, GfxCtx};

// TODO This is horrifically slow / memory inefficient. Reference implementation somewhere?

/// Draw freehand polygons
pub struct Lasso {
    points: Vec<Pt2D>,
    polygon: Option<Polygon>,
}

impl Lasso {
    pub fn new() -> Lasso {
        Lasso {
            points: Vec::new(),
            polygon: None,
        }
    }

    /// When this returns a polygon, the interaction is finished
    pub fn event(&mut self, ctx: &mut EventCtx) -> Option<Polygon> {
        if self.points.is_empty() {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if ctx.input.left_mouse_button_pressed() {
                    self.points.push(pt);
                }
            }
            return None;
        }

        if ctx.input.left_mouse_button_released() {
            return self.polygon.take();
        }

        if ctx.redo_mouseover() {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                if self.points.last().as_ref().unwrap().dist_to(pt) > Distance::meters(0.1) {
                    self.points.push(pt);

                    // TODO It's better if the user doesn't close the polygon themselves. When they
                    // try to, usually the result is the smaller polygon chunk
                    let mut copy = self.points.clone();
                    copy.push(copy[0]);
                    self.polygon = Ring::new(copy)
                        .ok()
                        .map(|ring| ring.into_polygon().simplify(1.0));
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if let Ok(pl) = PolyLine::new(self.points.clone()) {
            g.draw_polygon(
                Color::RED.alpha(0.8),
                pl.make_polygons(Distance::meters(5.0) / g.canvas.cam_zoom),
            );
        }
        if let Some(ref polygon) = self.polygon {
            g.draw_polygon(Color::RED.alpha(0.5), polygon.clone());
        }
    }
}

use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, ScreenPt, ScreenRectangle};
use geom::{Distance, Polygon, Pt2D, Ring};

pub struct Minimap {}

impl Minimap {
    pub fn new() -> Minimap {
        Minimap {}
    }

    pub fn event(&mut self, _: &mut UI, _: &mut EventCtx) {}

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            return;
        }

        // The background panel
        let square_len = 0.15 * g.canvas.window_width;
        let top_left = ScreenPt::new(
            g.canvas.window_width - square_len - 50.0,
            g.canvas.window_height - square_len - 50.0,
        );
        let bg = Polygon::rounded_rectangle(
            Distance::meters(square_len),
            Distance::meters(square_len),
            Distance::meters(10.0),
        )
        .translate(top_left.x, top_left.y);
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: top_left.x,
            x2: top_left.x + square_len,
            y1: top_left.y,
            y2: top_left.y + square_len,
        });
        g.fork_screenspace();
        g.draw_polygon(Color::grey(0.5), &bg);
        g.unfork();

        // The map
        g.fork(Pt2D::new(0.0, 0.0), top_left, 0.1);
        g.redraw(&ui.primary.draw_map.draw_all_areas);
        g.redraw(&ui.primary.draw_map.draw_all_thick_roads);
        g.redraw(&ui.primary.draw_map.draw_all_unzoomed_intersections);
        g.redraw(&ui.primary.draw_map.draw_all_buildings);

        // The cursor
        let bounds = ui.primary.map.get_bounds();
        let (x1, y1) = {
            let pt = g.canvas.screen_to_map(ScreenPt::new(0.0, 0.0));
            (
                clamp(pt.x(), 0.0, bounds.max_x),
                clamp(pt.y(), 0.0, bounds.max_y),
            )
        };
        let (x2, y2) = {
            let pt = g
                .canvas
                .screen_to_map(ScreenPt::new(g.canvas.window_width, g.canvas.window_height));
            (
                clamp(pt.x(), 0.0, bounds.max_x),
                clamp(pt.y(), 0.0, bounds.max_y),
            )
        };
        g.draw_polygon(
            Color::RED,
            &Ring::new(vec![
                Pt2D::new(x1, y1),
                Pt2D::new(x2, y1),
                Pt2D::new(x2, y2),
                Pt2D::new(x1, y2),
                Pt2D::new(x1, y1),
            ])
            .make_polygons(Distance::meters(20.0)),
        );
        g.unfork();
    }
}

fn clamp(x: f64, min: f64, max: f64) -> f64 {
    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}

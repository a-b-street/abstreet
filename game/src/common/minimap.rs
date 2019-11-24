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
        let padding = 10.0;
        let inner_rect = ScreenRectangle {
            x1: top_left.x + padding,
            x2: top_left.x + square_len - padding,
            y1: top_left.y + padding,
            y2: top_left.y + square_len - padding,
        };
        let bounds = ui.primary.map.get_bounds();
        // Fit the entire width of the map in the box, to start
        let zoom = (square_len - (padding * 2.0)) / (bounds.max_x - bounds.min_x);

        g.fork(
            Pt2D::new(0.0, 0.0),
            ScreenPt::new(inner_rect.x1, inner_rect.y1),
            zoom,
        );
        g.redraw_clipped(&ui.primary.draw_map.boundary_polygon, &inner_rect);
        g.redraw_clipped(&ui.primary.draw_map.draw_all_areas, &inner_rect);
        g.redraw_clipped(&ui.primary.draw_map.draw_all_thick_roads, &inner_rect);
        g.redraw_clipped(
            &ui.primary.draw_map.draw_all_unzoomed_intersections,
            &inner_rect,
        );
        g.redraw_clipped(&ui.primary.draw_map.draw_all_buildings, &inner_rect);

        // The cursor
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

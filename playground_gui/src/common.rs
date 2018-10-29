use ezgui::{Color, GfxCtx};
use geom::{Circle, PolyLine};

pub const WHITE: Color = Color([1.0; 4]);
pub const RED: Color = Color([1.0, 0.0, 0.0, 0.8]);
pub const GREEN: Color = Color([0.0, 1.0, 0.0, 0.8]);
pub const BLUE: Color = Color([0.0, 0.0, 1.0, 0.8]);
pub const BLACK: Color = Color([0.0, 0.0, 0.0, 0.3]);
pub const SOLID_BLACK: Color = Color([0.0, 0.0, 0.0, 0.9]);
pub const YELLOW: Color = Color([1.0, 1.0, 0.0, 0.8]);

pub fn draw_polyline(g: &mut GfxCtx, pl: &PolyLine, thickness: f64, color: Color) {
    for l in pl.lines() {
        g.draw_line(color, thickness, &l);
    }
    let radius = 0.5;
    let pts = pl.points();
    assert!(pts.len() >= 2);
    for pt in pts {
        g.draw_circle(BLUE, &Circle::new(*pt, radius));
    }
}

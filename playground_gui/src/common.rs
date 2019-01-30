use ezgui::{Color, GfxCtx};
use geom::{Circle, Distance, PolyLine};

// TODO Don't just use ezgui constants in this crate, since we want the slight transparency by
// default.
pub const WHITE: Color = Color::WHITE;
pub const RED: Color = Color::RED.alpha(0.8);
pub const GREEN: Color = Color::GREEN.alpha(0.8);
pub const BLUE: Color = Color::BLUE.alpha(0.8);
pub const BLACK: Color = Color::BLACK.alpha(0.3);
pub const SOLID_BLACK: Color = Color::BLACK.alpha(0.9);
pub const YELLOW: Color = Color::YELLOW.alpha(0.8);

pub fn draw_polyline(g: &mut GfxCtx, pl: &PolyLine, thickness: Distance, color: Color) {
    for l in pl.lines() {
        g.draw_line(color, thickness, &l);
    }
    let radius = Distance::meters(0.5);
    let pts = pl.points();
    assert!(pts.len() >= 2);
    for pt in pts {
        g.draw_circle(BLUE, &Circle::new(*pt, radius));
    }
}

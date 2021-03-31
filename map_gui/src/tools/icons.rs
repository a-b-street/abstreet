use geom::Pt2D;
use widgetry::{Color, GeomBatch, Prerender, RewriteColor};

/// Draw a start marker pointing at something.
pub fn start_marker<P: AsRef<Prerender>>(prerender: &P, pt: Pt2D, scale: f64) -> GeomBatch {
    GeomBatch::load_svg(prerender, "system/assets/timeline/start_pos.svg")
        .scale(scale)
        .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
        .color(RewriteColor::Change(
            Color::hex("#5B5B5B"),
            Color::hex("#CC4121"),
        ))
        .centered_on(pt)
        // Hand-tuned to make the tip of the icon point to the spot
        .translate(0.0, -10.0 * scale)
}

/// Draw a goal marker pointing at something.
pub fn goal_marker<P: AsRef<Prerender>>(prerender: &P, pt: Pt2D, scale: f64) -> GeomBatch {
    GeomBatch::load_svg(prerender, "system/assets/timeline/goal_pos.svg")
        .scale(scale)
        .color(RewriteColor::Change(Color::WHITE, Color::BLACK))
        .color(RewriteColor::Change(
            Color::hex("#5B5B5B"),
            Color::hex("#CC4121"),
        ))
        .centered_on(pt)
        // Hand-tuned to make the tip of the icon point to the spot
        .translate(8.0 * scale, -10.0 * scale)
}

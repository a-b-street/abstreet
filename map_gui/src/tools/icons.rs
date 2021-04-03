use geom::Pt2D;
use widgetry::{GeomBatch, Prerender, RewriteColor};

/// Draw a start marker pointing at something.
pub fn start_marker<P: AsRef<Prerender>>(prerender: &P, pt: Pt2D, scale: f64) -> GeomBatch {
    GeomBatch::load_svg(prerender, "system/assets/timeline/start_pos.svg")
        .scale(scale)
        .centered_on(pt)
        .color(RewriteColor::ChangeAlpha(0.8))
}

/// Draw a goal marker pointing at something.
pub fn goal_marker<P: AsRef<Prerender>>(prerender: &P, pt: Pt2D, scale: f64) -> GeomBatch {
    GeomBatch::load_svg(prerender, "system/assets/timeline/goal_pos.svg")
        .scale(scale)
        .centered_on(pt)
        .color(RewriteColor::ChangeAlpha(0.8))
}

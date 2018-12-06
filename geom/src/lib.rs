mod angle;
mod circle;
mod gps;
mod line;
mod polygon;
mod polyline;
mod pt;

pub use crate::angle::Angle;
pub use crate::circle::Circle;
pub use crate::gps::{GPSBounds, LonLat};
pub use crate::line::Line;
pub use crate::polygon::{Polygon, Triangle};
pub use crate::polyline::PolyLine;
pub use crate::pt::{Bounds, HashablePt2D, Pt2D};
use dimensioned::si;
use std::marker;

// About 0.4 inches... which is quite tiny on the scale of things. :)
pub const EPSILON_DIST: si::Meter<f64> = si::Meter {
    value_unsafe: 0.01,
    _marker: marker::PhantomData,
};

// NOT segment. Fails for parallel lines.
// https://en.wikipedia.org/wiki/Line%E2%80%93line_intersection#Given_two_points_on_each_line
pub fn line_intersection(l1: &Line, l2: &Line) -> Option<Pt2D> {
    let x1 = l1.pt1().x();
    let y1 = l1.pt1().y();
    let x2 = l1.pt2().x();
    let y2 = l1.pt2().y();

    let x3 = l2.pt1().x();
    let y3 = l2.pt1().y();
    let x4 = l2.pt2().x();
    let y4 = l2.pt2().y();

    let numer_x = (x1 * y2 - y1 * x2) * (x3 - x4) - (x1 - x2) * (x3 * y4 - y3 * x4);
    let numer_y = (x1 * y2 - y1 * x2) * (y3 - y4) - (y1 - y2) * (x3 * y4 - y3 * x4);
    let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
    if denom == 0.0 {
        None
    } else {
        Some(Pt2D::new(numer_x / denom, numer_y / denom))
    }
}

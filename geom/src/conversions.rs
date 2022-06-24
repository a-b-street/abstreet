//! Conversions between this crate and `geo`. Long-term, we should think about directly using `geo`
//! or wrapping it, but in the meantime...
//!
//! TODO Also, there's no consistency between standalone methods like this and From/Into impls.

use crate::Pt2D;

pub fn pts_to_line_string(raw_pts: &[Pt2D]) -> geo::LineString {
    let pts: Vec<geo::Point> = raw_pts
        .iter()
        .map(|pt| geo::Point::new(pt.x(), pt.y()))
        .collect();
    pts.into()
}

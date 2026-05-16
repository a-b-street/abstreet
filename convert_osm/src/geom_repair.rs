//! Resilient geometry builders for OSM polygon data.
//!
//! Volunteer-mapped and conflated OSM data contains two failure modes that
//! cause `Ring::new()` to panic or silently drop buildings:
//!
//! 1. **NaN/Inf coordinates** from GPS conflation tools — propagate into ring
//!    algebra and produce undefined results.
//! 2. **Bow-tie polygons** — way nodes ordered so the outline self-intersects.
//!    `Ring::deduping_new()` returns `Err` for these.
//!
//! This module provides `build_ring_resilient()` which filters non-finite
//! coordinates and falls back to the convex hull of the point cloud on ring
//! construction failure. The hull is always a valid convex polygon when >= 3
//! non-collinear points exist, and approximates the original area better than
//! dropping the entity entirely.

use geo::{ConvexHull, MultiPoint, Point};
use geom::{Pt2D, Ring};

/// Removes exact duplicate adjacent points and the trailing close-point.
fn remove_repeats(pts: &mut Vec<Pt2D>) {
    pts.dedup();
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
}

/// Retains only points with finite coordinates, logging each removal.
fn filter_non_finite(pts: &mut Vec<Pt2D>, context: &str) {
    let before = pts.len();
    pts.retain(|pt| {
        let ok = pt.x().is_finite() && pt.y().is_finite();
        if !ok {
            log::warn!(
                "Dropped non-finite coordinate ({}, {}) in {}",
                pt.x(),
                pt.y(),
                context
            );
        }
        ok
    });
    if pts.len() < before {
        log::warn!(
            "Filtered {} non-finite points from {}",
            before - pts.len(),
            context
        );
    }
}

/// Builds a `Ring` from raw OSM points, handling corrupted geometry gracefully.
///
/// Returns `None` only when the geometry is genuinely unrecoverable (fewer than
/// 3 finite non-collinear points). Logs a `WARN` for degraded geometry and an
/// `ERROR` for complete failures.
pub fn build_ring_resilient(mut pts: Vec<Pt2D>, context: &str) -> Option<Ring> {
    filter_non_finite(&mut pts, context);
    remove_repeats(&mut pts);

    if pts.len() < 3 {
        log::warn!(
            "Geometry failure for {}: only {} unique finite points -- dropping",
            context,
            pts.len()
        );
        return None;
    }

    // Happy path: standard construction handles most valid polygons.
    if let Ok(ring) = Ring::deduping_new(pts.clone()) {
        return Some(ring);
    }

    // Fallback: bow-tie or non-adjacent duplicate points detected.
    // Compute the convex hull, which is always a valid convex polygon for
    // >= 3 non-collinear points regardless of the original winding order.
    log::warn!(
        "Geometry degradation for {}: Ring construction failed (bow-tie polygon?). \
         Falling back to convex hull.",
        context
    );

    let hull: Vec<Pt2D> = MultiPoint(
        pts.iter()
            .map(|pt| Point::new(pt.x(), pt.y()))
            .collect::<Vec<_>>(),
    )
    .convex_hull()
    .exterior()
    .points()
    .map(|p| Pt2D::new(p.x(), p.y()))
    .collect();

    // The hull's exterior includes the closing duplicate point.
    let unique = if hull.len() > 1 && hull.first() == hull.last() {
        hull.len() - 1
    } else {
        hull.len()
    };

    if unique < 3 {
        log::error!(
            "Complete geometry failure for {}: convex hull degenerate \
             ({} unique pts) -- dropping",
            context,
            unique
        );
        return None;
    }

    match Ring::deduping_new(hull) {
        Ok(ring) => Some(ring),
        Err(e) => {
            log::error!(
                "Complete geometry failure for {}: convex hull Ring failed: \
                 {} -- dropping",
                context,
                e
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_ring_resilient;
    use geom::Pt2D;

    // Note: Pt2D::new() asserts finite values at construction, so NaN coordinate
    // paths are tested via integration test (Cairo import) rather than unit tests.

    fn sq() -> Vec<Pt2D> {
        vec![
            Pt2D::new(0.0, 0.0),
            Pt2D::new(10.0, 0.0),
            Pt2D::new(10.0, 10.0),
            Pt2D::new(0.0, 10.0),
        ]
    }

    #[test]
    fn valid_square_succeeds() {
        assert!(build_ring_resilient(sq(), "test").is_some());
    }

    #[test]
    fn two_points_returns_none() {
        let pts = vec![Pt2D::new(0.0, 0.0), Pt2D::new(5.0, 0.0)];
        assert!(build_ring_resilient(pts, "two_pts").is_none());
    }

    #[test]
    fn empty_returns_none() {
        assert!(build_ring_resilient(vec![], "empty").is_none());
    }

    #[test]
    fn bow_tie_falls_back_to_hull() {
        // Figure-8: nodes in self-intersecting order.
        // Ring::deduping_new will fail; convex hull should succeed.
        let pts = vec![
            Pt2D::new(0.0, 0.0),
            Pt2D::new(10.0, 10.0),
            Pt2D::new(10.0, 0.0),
            Pt2D::new(0.0, 10.0),
        ];
        assert!(build_ring_resilient(pts, "bow_tie").is_some());
    }
}

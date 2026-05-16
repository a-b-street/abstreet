//! NaN-safe fallback for turn arc geometry.
//!
//! `make_vehicle_turns()` falls back to `PolyLine::must_new([src, dst])` when
//! `curvey_turn()` fails. `must_new()` panics on two inputs that occur with
//! incomplete OSM data:
//!
//! - **NaN/Inf endpoints** propagated from corrupted upstream road geometry.
//! - **src == dst** at degenerate intersections where two lanes share an endpoint.
//!
//! `safe_straight_line()` handles all four cases and always returns a valid
//! `PolyLine`. Vehicle routing uses road center lines for pathfinding, not turn
//! arcs, so a stub fallback does not affect route computation — only the
//! intersection's visual arc is degraded.

use geom::{Angle, Distance, PolyLine, Pt2D};

/// Safely constructs the straight-line fallback for a turn's PolyLine geometry.
///
/// Returns a proper straight-line `PolyLine` when both endpoints are finite and
/// distinct. Otherwise synthesizes a 0.1 m North stub anchored at the nearest
/// valid point (or at the map origin when both endpoints are non-finite).
pub fn safe_straight_line(src: Pt2D, dst: Pt2D) -> PolyLine {
    let src_ok = src.x().is_finite() && src.y().is_finite();
    let dst_ok = dst.x().is_finite() && dst.y().is_finite();

    if !src_ok && !dst_ok {
        log::warn!(
            "Turn fallback: both endpoints non-finite ({},{}) -> ({},{}). Using origin stub.",
            src.x(),
            src.y(),
            dst.x(),
            dst.y()
        );
        return stub(Pt2D::new(0.0, 0.0));
    }

    if !src_ok {
        log::warn!(
            "Turn fallback: src ({},{}) non-finite. Stub from dst.",
            src.x(),
            src.y()
        );
        return stub(dst);
    }

    if !dst_ok {
        log::warn!(
            "Turn fallback: dst ({},{}) non-finite. Stub from src.",
            dst.x(),
            dst.y()
        );
        return stub(src);
    }

    // Both finite -- attempt normal construction.
    match PolyLine::new(vec![src, dst]) {
        Ok(pl) => pl,
        Err(_) => {
            // src and dst are identical or within floating-point epsilon.
            log::warn!(
                "Turn fallback: src == dst at ({},{}). Synthesizing stub.",
                src.x(),
                src.y()
            );
            stub(src)
        }
    }
}

fn stub(anchor: Pt2D) -> PolyLine {
    PolyLine::must_new(vec![
        anchor,
        anchor.project_away(Distance::meters(0.1), Angle::degrees(90.0)),
    ])
}

#[cfg(test)]
mod tests {
    use super::safe_straight_line;
    use geom::{Distance, Pt2D};

    // Note: Pt2D::new() asserts finite values at construction, so NaN coordinate
    // paths are tested via integration test (Cairo import) rather than unit tests.

    #[test]
    fn valid_endpoints_gives_straight_line() {
        let src = Pt2D::new(0.0, 0.0);
        let dst = Pt2D::new(5.0, 0.0);
        let pl = safe_straight_line(src, dst);
        // Should be a real line, not a 0.1m stub.
        assert!(pl.length() > Distance::meters(1.0));
    }

    #[test]
    fn identical_endpoints_returns_stub() {
        let pt = Pt2D::new(3.0, 7.0);
        let pl = safe_straight_line(pt, pt);
        assert!(pl.length() > Distance::meters(0.0));
    }
}

//! Overlap-based boundary clipping for buildings.
//!
//! The standard `clip_map` drops any building where a single vertex falls outside
//! the GPS boundary polygon. At city edges this silently removes buildings that are
//! 95% inside the map — the kind of thing you only notice when you zoom in on the
//! border and see a sudden gap in the building layer.
//!
//! `clip_map_resilient()` retains buildings with ≥5% area overlap instead.

use abstutil::Timer;
use raw_map::RawMap;

/// Replaces `clip_map`. Retains buildings with at least 5% overlap with the
/// boundary polygon instead of requiring all vertices to be inside.
pub fn clip_map_resilient(map: &mut RawMap, timer: &mut Timer) {
    let boundary_polygon = map.streets.boundary_polygon.clone();
    let min_overlap_threshold = 0.05; // 5%

    // Pre-compute boundary bounding box for cheap early rejection.
    // Buildings whose bbox doesn't overlap the boundary bbox can't overlap the boundary.
    let boundary_pts: Vec<_> = boundary_polygon.get_outer_ring().points().clone();
    let (bnd_min_x, bnd_max_x, bnd_min_y, bnd_max_y) = boundary_pts.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY),
        |(mn_x, mx_x, mn_y, mx_y), pt| {
            (mn_x.min(pt.x()), mx_x.max(pt.x()), mn_y.min(pt.y()), mx_y.max(pt.y()))
        },
    );

    map.buildings = timer.retain_parallelized(
        "resiliently clip buildings to boundary",
        std::mem::take(&mut map.buildings),
        |b| {
            let outer_pts: Vec<_> = b.polygon.get_outer_ring().points().clone();

            // Guard: degenerate polygon
            if outer_pts.len() < 4 {
                log::warn!("Building dropped: outer ring has {} points (degenerate)", outer_pts.len());
                return false;
            }

            // Guard: NaN coordinates
            if outer_pts.iter().any(|pt| !pt.x().is_finite() || !pt.y().is_finite()) {
                log::warn!("Building dropped: non-finite coordinates in polygon");
                return false;
            }

            // Fast path: all vertices inside — keep it
            if outer_pts.iter().all(|pt| boundary_polygon.contains_pt(*pt)) {
                return true;
            }

            // Guard: zero-area polygon — division would produce NaN
            let bldg_area = b.polygon.area();
            if bldg_area < 1e-9 {
                log::warn!("Building dropped: area {} is zero/negative (degenerate polygon)", bldg_area);
                return false;
            }

            // Cheap bounding box rejection before the expensive intersection test
            let (bldg_min_x, bldg_max_x, bldg_min_y, bldg_max_y) = outer_pts.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY),
                |(mn_x, mx_x, mn_y, mx_y), pt| {
                    (mn_x.min(pt.x()), mx_x.max(pt.x()), mn_y.min(pt.y()), mx_y.max(pt.y()))
                },
            );
            let bboxes_overlap = bldg_max_x >= bnd_min_x
                && bldg_min_x <= bnd_max_x
                && bldg_max_y >= bnd_min_y
                && bldg_min_y <= bnd_max_y;
            if !bboxes_overlap {
                return false;
            }

            // Polygon intersection: only reached for buildings near the boundary
            if let Ok(intersections) = boundary_polygon.intersection(&b.polygon) {
                let overlap_area: f64 = intersections.iter()
                    .map(|p| p.area().abs())
                    .sum();
                if overlap_area / bldg_area >= min_overlap_threshold {
                    return true;
                }
            }

            false
        },
    );

    map.areas = timer
        .parallelize(
            "clip areas to boundary",
            std::mem::take(&mut map.areas),
            |orig_area| {
                let mut result = Vec::new();
                let pts: Vec<_> = orig_area.polygon.get_outer_ring().points().clone();
                if pts.len() < 4 {
                    return result;
                }
                if let Ok(list) = map
                    .streets
                    .boundary_polygon
                    .intersection(&orig_area.polygon)
                {
                    for polygon in list {
                        let mut area = orig_area.clone();
                        area.polygon = polygon;
                        result.push(area);
                    }
                }
                result
            },
        )
        .into_iter()
        .flatten()
        .collect();
}

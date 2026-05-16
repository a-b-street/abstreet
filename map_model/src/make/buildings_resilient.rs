//! 4-tier building snap cascade for sidewalk-sparse cities.
//!
//! `make_all_buildings()` calls `match_points_to_lanes` with `|l| l.is_walkable()`
//! and a 1km radius. In cities where sidewalks and footways aren't mapped in OSM,
//! this drops every building that has no walkable lane within 1km — which in
//! poorly-surveyed areas can be most of the city.
//!
//! `snap_buildings_resilient()` cascades through four strategies, logging how
//! many buildings needed each tier so the import summary gives a clear picture
//! of data quality.

use abstutil::Timer;
use geom::{Distance, HashablePt2D};
use std::collections::{HashMap, HashSet};
use crate::{Map, Position};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SnapStrategy {
    NearestSidewalk,
    ExtendedSidewalk,
    RoadEdgeInferred,
    Stub,
}

impl SnapStrategy {
    pub fn radius(&self) -> Distance {
        match self {
            SnapStrategy::NearestSidewalk => Distance::meters(30.0),
            SnapStrategy::ExtendedSidewalk => Distance::meters(80.0),
            SnapStrategy::RoadEdgeInferred => Distance::meters(150.0),
            SnapStrategy::Stub => Distance::meters(1000.0),
        }
    }
}

/// Replacement for the single-pass `match_points_to_lanes` call in `make_all_buildings`.
///
/// Tier 1: 30m  nearest sidewalk/footway (strict — same criteria as existing code)
/// Tier 2: 80m  extended sidewalk search
/// Tier 3: 150m any non-light-rail lane (road edge inferred, no sidewalk required)
/// Tier 4: 1km  stub snap to distant road
///
/// Buildings only drop if all four tiers fail.
pub fn snap_buildings_resilient(
    map: &Map,
    query: HashSet<HashablePt2D>,
    timer: &mut Timer,
) -> HashMap<HashablePt2D, Position> {
    timer.start("resilient 4-tier snap building cascade");

    let total = query.len();

    // Tier 1: 30m nearest sidewalk
    let mut results = crate::make::match_points_to_lanes(
        map,
        query.clone(),
        |l| l.is_walkable(),
        Distance::meters(1.0),
        SnapStrategy::NearestSidewalk.radius(),
        timer,
    );
    let t1_count = results.len();

    let mut missing: HashSet<HashablePt2D> = query.difference(&results.keys().cloned().collect()).cloned().collect();

    if !missing.is_empty() {
        // Tier 2: 80m extended sidewalk
        let t2 = crate::make::match_points_to_lanes(
            map,
            missing.clone(),
            |l| l.is_walkable(),
            Distance::meters(1.0),
            SnapStrategy::ExtendedSidewalk.radius(),
            timer,
        );
        if !t2.is_empty() {
            log::info!("Snap Tier-2 (80m sidewalk): {} buildings snapped via extended radius.", t2.len());
        }
        missing.retain(|pt| !t2.contains_key(pt));
        results.extend(t2);
    }

    if !missing.is_empty() {
        // Tier 3: 150m road edge (any non-light-rail)
        let t3 = crate::make::match_points_to_lanes(
            map,
            missing.clone(),
            |l| !l.is_light_rail(),
            Distance::meters(1.0),
            SnapStrategy::RoadEdgeInferred.radius(),
            timer,
        );
        if !t3.is_empty() {
            log::warn!("Snap Tier-3 (150m road edge): {} buildings snapped without sidewalk.", t3.len());
        }
        missing.retain(|pt| !t3.contains_key(pt));
        results.extend(t3);
    }

    if !missing.is_empty() {
        // Tier 4: 1km stub
        let t4 = crate::make::match_points_to_lanes(
            map,
            missing.clone(),
            |l| !l.is_light_rail(),
            Distance::meters(0.0),
            SnapStrategy::Stub.radius(),
            timer,
        );
        if !t4.is_empty() {
            log::warn!("Snap Tier-4 (1km stub): {} buildings snapped to distant road.", t4.len());
        }
        results.extend(t4);
    }

    timer.stop("resilient 4-tier snap building cascade");

    let snapped = results.len();
    let dropped = total - snapped;
    log::info!(
        "Building snap: {}/{} snapped (T1={} degraded={} dropped={})",
        snapped, total, t1_count, snapped - t1_count, dropped
    );
    if dropped > 0 {
        log::warn!("{} buildings completely un-routable after all 4 tiers.", dropped);
    }

    results
}

//! Tag harmonization for short degenerate road segments before topology merge.
//!
//! Volunteer-mapped OSM data frequently has short connector segments (<15m) at
//! intersections where two roads from different mapping sessions meet. These
//! segments represent the same road as their longer neighbour but have mismatched
//! `name`, `highway_type`, or `lane_specs_ltr` because different contributors
//! surveyed them.
//!
//! When `apply_transformations(Transformation::abstreet())` merges degenerate
//! intersections (exactly 2 roads meeting), tag disagreement can cause the wrong
//! values to win. `detect_merge_candidates()` harmonizes the tags on the short
//! road before the merge so the longer road's values take effect.

use geom::{Angle, Distance};
use osm2streets::StreetNetwork;

pub struct MergeCandidateConfig {
    /// Maximum length for a road to be considered a merge candidate.
    pub max_length: Distance,
    /// Maximum bearing change between the two roads — skips U-turns.
    pub max_bearing_change: Angle,
    /// If true, only merge when all tags already match (dry-run mode).
    pub exact_match_required: bool,
}

impl Default for MergeCandidateConfig {
    fn default() -> Self {
        Self {
            max_length: Distance::meters(15.0),
            max_bearing_change: Angle::degrees(25.0),
            exact_match_required: false,
        }
    }
}

/// Harmonize tags on short degenerate road segments before `apply_transformations`.
///
/// Call this immediately before `raw.streets.apply_transformations(Transformation::abstreet(), timer)`.
pub fn detect_merge_candidates(streets: &mut StreetNetwork, config: MergeCandidateConfig) {
    let mut harmonized = 0;

    // Snapshot first — mutations during iteration would invalidate the iterator.
    let candidates: Vec<_> = streets
        .intersections
        .values()
        .filter(|i| i.roads.len() == 2)
        .map(|i| (i.id, i.roads[0], i.roads[1]))
        .collect();

    for (_i_id, r1_id, r2_id) in candidates {
        // Guard: prior iterations may have modified the network
        if !streets.roads.contains_key(&r1_id) || !streets.roads.contains_key(&r2_id) {
            continue;
        }

        let (len1, len2, skip) = {
            let r1 = &streets.roads[&r1_id];
            let r2 = &streets.roads[&r2_id];

            // Skip roads with turn restrictions — merging would discard them
            if !r1.turn_restrictions.is_empty()
                || !r1.complicated_turn_restrictions.is_empty()
                || !r2.turn_restrictions.is_empty()
                || !r2.complicated_turn_restrictions.is_empty()
            {
                continue;
            }

            let l1 = r1.center_line.length();
            let l2 = r2.center_line.length();

            // Guard: zero-length roads produce undefined bearing — skip the angle check
            let skip_bearing = l1 < Distance::meters(0.1) || l2 < Distance::meters(0.1);

            let should_skip = if !skip_bearing {
                let b1 = r1.center_line.first_pt()
                    .angle_to(r1.center_line.last_pt())
                    .normalized_degrees();
                let b2 = r2.center_line.first_pt()
                    .angle_to(r2.center_line.last_pt())
                    .normalized_degrees();
                let raw_delta = (b1 - b2).abs();
                let delta = if raw_delta > 180.0 { 360.0 - raw_delta } else { raw_delta };
                delta > config.max_bearing_change.normalized_degrees()
            } else {
                false
            };

            (l1, l2, should_skip)
        };

        if skip {
            continue;
        }

        // Identify short/long road pair
        let (short_id, long_id) = if len1 < config.max_length && len1 <= len2 {
            (r1_id, r2_id)
        } else if len2 < config.max_length && len2 <= len1 {
            (r2_id, r1_id)
        } else {
            continue;
        };

        if !streets.roads.contains_key(&long_id) || !streets.roads.contains_key(&short_id) {
            continue;
        }

        let long_road = streets.roads[&long_id].clone();
        if let Some(short_road) = streets.roads.get_mut(&short_id) {
            let mut changed = false;

            if !config.exact_match_required {
                if short_road.name != long_road.name {
                    short_road.name = long_road.name.clone();
                    changed = true;
                }
                if short_road.highway_type != long_road.highway_type {
                    short_road.highway_type = long_road.highway_type.clone();
                    changed = true;
                }
                if short_road.lane_specs_ltr != long_road.lane_specs_ltr {
                    short_road.lane_specs_ltr = long_road.lane_specs_ltr.clone();
                    changed = true;
                }
            }
            if short_road.layer != long_road.layer {
                short_road.layer = long_road.layer;
                changed = true;
            }
            if short_road.reference_line_placement != long_road.reference_line_placement {
                short_road.reference_line_placement = long_road.reference_line_placement.clone();
                changed = true;
            }

            if changed {
                harmonized += 1;
            }
        }
    }

    if harmonized > 0 {
        log::info!(
            "Topology repair: harmonized tags on {} short degenerate road segments.",
            harmonized
        );
    }
}

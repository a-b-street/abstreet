use anyhow::Result;
use osm2lanes::road::Designated;

use abstutil::Tags;
use geom::Distance;

use crate::{osm, Direction, DrivingSide, LaneSpec, LaneType, MapConfig};

pub fn get_lane_specs_ltr(orig_tags: &Tags, cfg: &MapConfig) -> Vec<LaneSpec> {
    // Special cases first
    if orig_tags.is_any("railway", vec!["light_rail", "rail"]) {
        return vec![LaneSpec {
            lt: LaneType::LightRail,
            dir: Direction::Fwd,
            width: LaneSpec::typical_lane_widths(LaneType::LightRail, orig_tags)[0].0,
        }];
    }

    match inner_get_lane_specs_ltr(orig_tags, cfg) {
        Ok(lanes) => lanes,
        Err(err) => {
            let way_id = orig_tags.get(osm::OSM_WAY_ID).unwrap();
            error!(
                "osm2lanes broke on https://www.openstreetmap.org/way/{} with tags {:?}: {}",
                way_id, orig_tags, err
            );
            vec![LaneSpec {
                lt: LaneType::Driving,
                dir: Direction::Fwd,
                width: Distance::meters(1.0),
            }]
        }
    }
}

fn inner_get_lane_specs_ltr(orig_tags: &Tags, cfg: &MapConfig) -> Result<Vec<LaneSpec>> {
    let tags = transform_tags(orig_tags, cfg);
    let locale = osm2lanes::locale::Config::new()
        .driving_side(match cfg.driving_side {
            DrivingSide::Right => osm2lanes::locale::DrivingSide::Right,
            DrivingSide::Left => osm2lanes::locale::DrivingSide::Left,
        })
        .build();
    let mut config = osm2lanes::transform::TagsToLanesConfig::default();
    config.error_on_warnings = false;
    config.include_separators = true;

    let output = osm2lanes::transform::tags_to_lanes(&tags, &locale, &config)?;
    let highway_type = output.road.highway.r#type();

    let mut result = Vec::new();
    for lane in output.road.lanes {
        let mut new_lanes = transform_lane(lane, &locale, highway_type, cfg, result.is_empty())?;
        if new_lanes.is_empty() {
            continue;
        }

        // Don't use widths from osm2lanes yet
        for lane in &mut new_lanes {
            lane.width = LaneSpec::typical_lane_widths(lane.lt, &orig_tags)[0].0;
        }

        // If we split a bidirectional lane into two pieces, halve the width of each piece
        if new_lanes.len() == 2 {
            for lane in &mut new_lanes {
                lane.width *= 0.5;
            }
        }

        result.extend(new_lanes);
    }

    // No shoulders on unwalkable roads
    if orig_tags.is_any(
        crate::osm::HIGHWAY,
        vec!["motorway", "motorway_link", "construction"],
    ) || orig_tags.is("foot", "no")
        || orig_tags.is("access", "no")
        || orig_tags.is("motorroad", "yes")
    {
        result.retain(|lane| lane.lt != LaneType::Shoulder);
    }

    if output.road.highway.is_construction() {
        // Remove sidewalks and make everything else a construction lane
        result.retain(|lane| !lane.lt.is_walkable());
        for lane in &mut result {
            lane.lt = LaneType::Construction;
        }
    }

    // If there's no driving lane, ignore any assumptions about parking
    // (https://www.openstreetmap.org/way/6449188 is an example)
    if result.iter().all(|lane| lane.lt != LaneType::Driving) {
        result.retain(|lane| lane.lt != LaneType::Parking);
    }

    if let Some(x) = orig_tags
        .get("sidewalk:left:width")
        .and_then(|num| num.parse::<f64>().ok())
    {
        // TODO Make sure this is a sidewalk!
        result[0].width = Distance::meters(x);
    }
    if let Some(x) = orig_tags
        .get("sidewalk:right:width")
        .and_then(|num| num.parse::<f64>().ok())
    {
        result.last_mut().unwrap().width = Distance::meters(x);
    }

    // Fix direction on outer lanes
    for (idx, lane) in result.iter_mut().enumerate() {
        if lane.lt == LaneType::Sidewalk || lane.lt == LaneType::Shoulder {
            if idx == 0 {
                lane.dir = if cfg.driving_side == DrivingSide::Right {
                    Direction::Back
                } else {
                    Direction::Fwd
                };
            } else {
                // Assume last
                lane.dir = if cfg.driving_side == DrivingSide::Right {
                    Direction::Fwd
                } else {
                    Direction::Back
                };
            }
        }
    }

    Ok(result)
}

fn transform_tags(tags: &Tags, cfg: &MapConfig) -> osm_tags::Tags {
    let mut tags = tags.clone();

    // Patch around some common issues
    if tags.is(osm::SIDEWALK, "none") {
        tags.insert(osm::SIDEWALK, "no");
    }
    if tags.is("oneway", "reversible") {
        tags.insert("oneway", "yes");
    }
    if tags.is("highway", "living_street") {
        tags.insert("highway", "residential");
    }

    if tags.is(osm::SIDEWALK, "separate") && cfg.inferred_sidewalks {
        // Make blind guesses
        let value = if tags.is("oneway", "yes") {
            if cfg.driving_side == DrivingSide::Right {
                "right"
            } else {
                "left"
            }
        } else {
            "both"
        };
        tags.insert(osm::SIDEWALK, value);
    }

    // If there's no sidewalk data in OSM already, then make an assumption and mark that it's
    // inferred.
    if !tags.contains_key(osm::SIDEWALK) && cfg.inferred_sidewalks {
        tags.insert(osm::INFERRED_SIDEWALKS, "true");

        if tags.contains_key("sidewalk:left") || tags.contains_key("sidewalk:right") {
            // Attempt to mangle
            // https://wiki.openstreetmap.org/wiki/Key:sidewalk#Separately_mapped_sidewalks_on_only_one_side
            // into left/right/both. We have to make assumptions for missing values.
            let right = !tags.is("sidewalk:right", "no");
            let left = !tags.is("sidewalk:left", "no");
            let value = match (right, left) {
                (true, true) => "both",
                (true, false) => "right",
                (false, true) => "left",
                (false, false) => "no",
            };
            tags.insert(osm::SIDEWALK, value);
            // Remove conflicting values
            tags.remove("sidewalk:right");
            tags.remove("sidewalk:left");
        } else if tags.is_any(osm::HIGHWAY, vec!["motorway", "motorway_link"])
            || tags.is_any("junction", vec!["intersection", "roundabout"])
            || tags.is("foot", "no")
            || tags.is(osm::HIGHWAY, "service")
            // TODO For now, not attempting shared walking/biking paths.
            || tags.is_any(osm::HIGHWAY, vec!["cycleway", "pedestrian", "track"])
        {
            tags.insert(osm::SIDEWALK, "no");
        } else if tags.is("oneway", "yes") {
            if cfg.driving_side == DrivingSide::Right {
                tags.insert(osm::SIDEWALK, "right");
            } else {
                tags.insert(osm::SIDEWALK, "left");
            }
            if tags.is_any(osm::HIGHWAY, vec!["residential", "living_street"])
                && !tags.is("dual_carriageway", "yes")
            {
                tags.insert(osm::SIDEWALK, "both");
            }
        } else {
            tags.insert(osm::SIDEWALK, "both");
        }
    }

    // Multiple bus schemas
    if tags.has_any(vec!["bus:lanes:forward", "bus:lanes:backward"])
        && tags.has_any(vec!["lanes:bus:forward", "lanes:bus:backward"])
    {
        // Arbitrarily pick one!
        tags.remove("lanes:bus:forward");
        tags.remove("lanes:bus:backward");
    }

    // Nothing supports the concept of contraflow cycling without an explicit lane yet, so just
    // ignore this
    if tags.is("cycleway", "opposite")
        && tags.is("oneway", "yes")
        && !tags.is("oneway:bicycle", "no")
    {
        tags.remove("cycleway");
    }

    // Bidirectional 1 lane roads not modelled yet
    if tags.is("lanes", "1") && !tags.is("oneway", "yes") {
        tags.insert("lanes", "2");
    }

    let mut result = osm_tags::Tags::default();
    for (k, v) in tags.inner() {
        result.checked_insert(k.to_string(), v).unwrap();
    }
    result
}

// This produces:
// - 0 lanes if we're ignoring this lane entirely (a separator)
// - 1 lane in most cases
// - 2 lanes if we're splitting a bidirectional lane
fn transform_lane(
    lane: osm2lanes::road::Lane,
    locale: &osm2lanes::locale::Locale,
    highway_type: osm_tags::HighwayType,
    cfg: &MapConfig,
    is_first_lane: bool,
) -> Result<Vec<LaneSpec>> {
    use osm2lanes::road::Lane;

    let single_lane = |lt, dir| {
        let width = Distance::meters(lane.width(locale, highway_type).val());
        Ok(vec![LaneSpec { lt, dir, width }])
    };

    match lane {
        Lane::Travel {
            direction,
            designated,
            ..
        } => {
            let lt = match designated {
                Designated::Foot => LaneType::Sidewalk,
                Designated::Motor => LaneType::Driving,
                Designated::Bicycle => LaneType::Biking,
                Designated::Bus => LaneType::Bus,
            };
            if let Some(dir) = match direction {
                Some(osm2lanes::road::Direction::Forward) => Some(Direction::Fwd),
                Some(osm2lanes::road::Direction::Backward) => Some(Direction::Back),
                Some(osm2lanes::road::Direction::Both) => None,
                // We'll fix direction of outermost sidewalks/shoulders later
                None => Some(Direction::Fwd),
            } {
                return single_lane(lt, dir);
            }

            // Direction = both gets more complicated.

            // If this isn't the first / leftmost lane and it's bidi for cars, then that's a shared
            // turn lane. We may change the osm2lanes representation to clarify this
            // (https://github.com/a-b-street/osm2lanes/issues/184).
            if lt == LaneType::Driving && !is_first_lane {
                return single_lane(LaneType::SharedLeftTurn, Direction::Fwd);
            }
            if lt == LaneType::Sidewalk {
                bail!("Unexpected direction=both and designated=foot");
            }

            // Otherwise, represent the bidirection car/bike/bus lane as two half-width lanes
            let total_width = Distance::meters(lane.width(locale, highway_type).val());
            Ok(bidirectional_lane(lt, total_width, cfg))
        }
        Lane::Shoulder { .. } => {
            // We'll fix direction of outermost sidewalks/shoulders later
            single_lane(LaneType::Shoulder, Direction::Fwd)
        }
        Lane::Separator { .. } => {
            // TODO Barriers?
            Ok(Vec::new())
        }
        Lane::Parking {
            direction,
            designated: Designated::Motor,
            ..
        } => {
            let dir = match direction {
                osm2lanes::road::Direction::Forward => Direction::Fwd,
                osm2lanes::road::Direction::Backward => Direction::Back,
                osm2lanes::road::Direction::Both => bail!("dir = both for parking"),
            };
            single_lane(LaneType::Parking, dir)
        }
        _ => bail!("handle {:?}", lane),
    }
}

// Transform one lane into two, since A/B Street can't properly model narrow lanes
fn bidirectional_lane(lt: LaneType, total_width: Distance, cfg: &MapConfig) -> Vec<LaneSpec> {
    let (dir1, dir2) = if cfg.driving_side == DrivingSide::Right {
        (Direction::Back, Direction::Fwd)
    } else {
        (Direction::Fwd, Direction::Back)
    };
    vec![
        LaneSpec {
            lt,
            dir: dir1,
            width: total_width / 2.0,
        },
        LaneSpec {
            lt,
            dir: dir2,
            width: total_width / 2.0,
        },
    ]
}

/// Purely from OSM tags, determine the lanes that a road segment has.
use std::iter;

use abstutil::Tags;

use crate::{osm, BufferType, Direction, DrivingSide, LaneSpec, LaneType, MapConfig};

pub fn get_lane_specs_ltr(tags: &Tags, cfg: &MapConfig) -> Vec<LaneSpec> {
    let fwd = |lt: LaneType| LaneSpec {
        lt,
        dir: Direction::Fwd,
        width: LaneSpec::typical_lane_widths(lt, tags)[0].0,
    };
    let back = |lt: LaneType| LaneSpec {
        lt,
        dir: Direction::Back,
        width: LaneSpec::typical_lane_widths(lt, tags)[0].0,
    };

    // Easy special cases first.
    if tags.is_any("railway", vec!["light_rail", "rail"]) {
        return vec![fwd(LaneType::LightRail)];
    }
    if tags.is(osm::HIGHWAY, "steps") {
        return vec![fwd(LaneType::Sidewalk)];
    }
    // Eventually, we should have some kind of special LaneType for shared walking/cycling paths of
    // different kinds. Until then, model by making bike lanes and a shoulder for walking.
    if tags.is_any(
        osm::HIGHWAY,
        vec!["cycleway", "footway", "path", "pedestrian", "track"],
    ) {
        // If it just allows foot traffic, simply make it a sidewalk. For most of the above highway
        // types, assume bikes are allowed, except for footways, where they must be explicitly
        // allowed.
        if tags.is("bicycle", "no")
            || (tags.is(osm::HIGHWAY, "footway")
                && !tags.is_any("bicycle", vec!["designated", "yes", "dismount"]))
        {
            return vec![fwd(LaneType::Sidewalk)];
        }
        // Otherwise, there'll always be a bike lane.

        let mut fwd_side = vec![fwd(LaneType::Biking)];
        let mut back_side = if tags.is("oneway", "yes") {
            vec![]
        } else {
            vec![back(LaneType::Biking)]
        };

        if !tags.is("foot", "no") {
            fwd_side.push(fwd(LaneType::Shoulder));
            if !back_side.is_empty() {
                back_side.push(back(LaneType::Shoulder));
            }
        }
        return LaneSpec::assemble_ltr(fwd_side, back_side, cfg.driving_side);
    }

    // TODO Reversible roads should be handled differently?
    let oneway =
        tags.is_any("oneway", vec!["yes", "reversible"]) || tags.is("junction", "roundabout");

    // How many driving lanes in each direction?
    let num_driving_fwd = if let Some(n) = tags
        .get("lanes:forward")
        .and_then(|num| num.parse::<usize>().ok())
    {
        n
    } else if let Some(n) = tags.get("lanes").and_then(|num| num.parse::<usize>().ok()) {
        if oneway {
            n
        } else if n % 2 == 0 {
            n / 2
        } else {
            // usize division rounds down
            (n / 2) + 1
        }
    } else {
        1
    };
    let num_driving_back = if let Some(n) = tags
        .get("lanes:backward")
        .and_then(|num| num.parse::<usize>().ok())
    {
        n
    } else if let Some(n) = tags.get("lanes").and_then(|num| num.parse::<usize>().ok()) {
        let base = n - num_driving_fwd;
        if oneway {
            base
        } else {
            // lanes=1 but not oneway... what is this supposed to mean?
            base.max(1)
        }
    } else if oneway {
        0
    } else {
        1
    };

    #[allow(clippy::if_same_then_else)] // better readability
    let driving_lane =
        if tags.is("access", "no") && (tags.is("bus", "yes") || tags.is("psv", "yes")) {
            // Sup West Seattle
            LaneType::Bus
        } else if tags
            .get("motor_vehicle:conditional")
            .map(|x| x.starts_with("no"))
            .unwrap_or(false)
            && tags.is("bus", "yes")
        {
            // Example: 3rd Ave in downtown Seattle
            LaneType::Bus
        } else if tags.is("access", "no") || tags.is("highway", "construction") {
            LaneType::Construction
        } else {
            LaneType::Driving
        };

    // These are ordered from the road center, going outwards. Most of the members of fwd_side will
    // have Direction::Fwd, but there can be exceptions with two-way cycletracks.
    let mut fwd_side: Vec<LaneSpec> = iter::repeat_with(|| fwd(driving_lane))
        .take(num_driving_fwd)
        .collect();
    let mut back_side: Vec<LaneSpec> = iter::repeat_with(|| back(driving_lane))
        .take(num_driving_back)
        .collect();
    // TODO Fix upstream. https://wiki.openstreetmap.org/wiki/Key:centre_turn_lane
    if tags.is("lanes:both_ways", "1") || tags.is("centre_turn_lane", "yes") {
        fwd_side.insert(0, fwd(LaneType::SharedLeftTurn));
    }

    if driving_lane == LaneType::Construction {
        return LaneSpec::assemble_ltr(fwd_side, back_side, cfg.driving_side);
    }

    let fwd_bus_spec = if let Some(s) = tags.get("bus:lanes:forward") {
        s
    } else if let Some(s) = tags.get("psv:lanes:forward") {
        s
    } else if oneway {
        if let Some(s) = tags.get("bus:lanes") {
            s
        } else if let Some(s) = tags.get("psv:lanes") {
            s
        } else {
            ""
        }
    } else {
        ""
    };
    if !fwd_bus_spec.is_empty() {
        let parts: Vec<&str> = fwd_bus_spec.split('|').collect();
        let offset = if fwd_side[0].lt == LaneType::SharedLeftTurn {
            1
        } else {
            0
        };
        if parts.len() == fwd_side.len() - offset {
            for (idx, part) in parts.into_iter().enumerate() {
                if part == "designated" {
                    fwd_side[idx + offset].lt = LaneType::Bus;
                }
            }
        }
    }
    if let Some(spec) = tags
        .get("bus:lanes:backward")
        .or_else(|| tags.get("psv:lanes:backward"))
    {
        let parts: Vec<&str> = spec.split('|').collect();
        if parts.len() == back_side.len() {
            for (idx, part) in parts.into_iter().enumerate() {
                if part == "designated" {
                    back_side[idx].lt = LaneType::Bus;
                }
            }
        }
    }

    if tags.is_any("cycleway", vec!["lane", "track"]) {
        fwd_side.push(fwd(LaneType::Biking));
        if !back_side.is_empty() {
            back_side.push(back(LaneType::Biking));
        }
    } else if tags.is_any("cycleway:both", vec!["lane", "track"]) {
        fwd_side.push(fwd(LaneType::Biking));
        back_side.push(back(LaneType::Biking));
    } else {
        // Note here that we look at driving_side frequently, to match up left/right with fwd/back.
        // If we're driving on the right, then right=fwd. Driving on the left, then right=back.
        //
        // TODO Can we express this more simply by referring to a left_side and right_side here?
        if tags.is_any("cycleway:right", vec!["lane", "track"]) {
            if cfg.driving_side == DrivingSide::Right {
                if tags.is("cycleway:right:oneway", "no") || tags.is("oneway:bicycle", "no") {
                    fwd_side.push(back(LaneType::Biking));
                }
                fwd_side.push(fwd(LaneType::Biking));
            } else {
                if tags.is("cycleway:right:oneway", "no") || tags.is("oneway:bicycle", "no") {
                    back_side.push(fwd(LaneType::Biking));
                }
                back_side.push(back(LaneType::Biking));
            }
        }
        if tags.is("cycleway:left", "opposite_lane") || tags.is("cycleway", "opposite_lane") {
            if cfg.driving_side == DrivingSide::Right {
                back_side.push(back(LaneType::Biking));
            } else {
                fwd_side.push(fwd(LaneType::Biking));
            }
        }
        if tags.is_any("cycleway:left", vec!["lane", "opposite_track", "track"]) {
            if cfg.driving_side == DrivingSide::Right {
                if tags.is("cycleway:left:oneway", "no") || tags.is("oneway:bicycle", "no") {
                    back_side.push(fwd(LaneType::Biking));
                    back_side.push(back(LaneType::Biking));
                } else if oneway {
                    fwd_side.insert(0, fwd(LaneType::Biking));
                } else {
                    back_side.push(back(LaneType::Biking));
                }
            } else {
                // TODO This should mimic the logic for right-handed driving, but I need test cases
                // first to do this sanely
                if tags.is("cycleway:left:oneway", "no") || tags.is("oneway:bicycle", "no") {
                    fwd_side.push(back(LaneType::Biking));
                }
                fwd_side.push(fwd(LaneType::Biking));
            }
        }
    }

    // My brain hurts. How does the above combinatorial explosion play with
    // https://wiki.openstreetmap.org/wiki/Proposed_features/cycleway:separation? Let's take the
    // "post-processing" approach.
    // TODO Not attempting left-handed driving yet.
    // TODO A two-way cycletrack on one side of a one-way road will almost definitely break this.
    if let Some(buffer) = tags
        .get("cycleway:right:separation:left")
        .and_then(osm_separation_type)
    {
        // TODO These shouldn't fail, but snapping is imperfect... like around
        // https://www.openstreetmap.org/way/486283205
        if let Some(idx) = fwd_side.iter().position(|x| x.lt == LaneType::Biking) {
            fwd_side.insert(idx, fwd(LaneType::Buffer(buffer)));
        }
    }
    if let Some(buffer) = tags
        .get("cycleway:left:separation:left")
        .and_then(osm_separation_type)
    {
        if let Some(idx) = back_side.iter().position(|x| x.lt == LaneType::Biking) {
            back_side.insert(idx, back(LaneType::Buffer(buffer)));
        }
    }
    if let Some(buffer) = tags
        .get("cycleway:left:separation:right")
        .and_then(osm_separation_type)
    {
        // This is assuming a one-way road. That's why we're not looking at back_side.
        if let Some(idx) = fwd_side.iter().position(|x| x.lt == LaneType::Biking) {
            fwd_side.insert(idx + 1, fwd(LaneType::Buffer(buffer)));
        }
    }

    if driving_lane == LaneType::Driving {
        let has_parking = vec!["parallel", "diagonal", "perpendicular"];
        let parking_lane_fwd = tags.is_any(osm::PARKING_RIGHT, has_parking.clone())
            || tags.is_any(osm::PARKING_BOTH, has_parking.clone());
        let parking_lane_back = tags.is_any(osm::PARKING_LEFT, has_parking.clone())
            || tags.is_any(osm::PARKING_BOTH, has_parking);
        if parking_lane_fwd {
            fwd_side.push(fwd(LaneType::Parking));
        }
        if parking_lane_back {
            back_side.push(back(LaneType::Parking));
        }
    }

    if tags.is(osm::SIDEWALK, "both") {
        fwd_side.push(fwd(LaneType::Sidewalk));
        back_side.push(back(LaneType::Sidewalk));
    } else if tags.is(osm::SIDEWALK, "separate") && cfg.inferred_sidewalks {
        // TODO Need to snap separate sidewalks to ways. Until then, just do this.
        fwd_side.push(fwd(LaneType::Sidewalk));
        if !back_side.is_empty() {
            back_side.push(back(LaneType::Sidewalk));
        }
    } else if tags.is(osm::SIDEWALK, "right") {
        if cfg.driving_side == DrivingSide::Right {
            fwd_side.push(fwd(LaneType::Sidewalk));
        } else {
            back_side.push(back(LaneType::Sidewalk));
        }
    } else if tags.is(osm::SIDEWALK, "left") {
        if cfg.driving_side == DrivingSide::Right {
            back_side.push(back(LaneType::Sidewalk));
        } else {
            fwd_side.push(fwd(LaneType::Sidewalk));
        }
    }

    let mut need_fwd_shoulder = fwd_side
        .last()
        .map(|spec| spec.lt != LaneType::Sidewalk)
        .unwrap_or(true);
    let mut need_back_shoulder = back_side
        .last()
        .map(|spec| spec.lt != LaneType::Sidewalk)
        .unwrap_or(true);
    if tags.is_any(
        osm::HIGHWAY,
        vec!["motorway", "motorway_link", "construction"],
    ) || tags.is("foot", "no")
        || tags.is("access", "no")
        || tags.is("motorroad", "yes")
    {
        need_fwd_shoulder = false;
        need_back_shoulder = false;
    }
    // If it's a one-way, fine to not have sidewalks on both sides.
    if tags.is("oneway", "yes") {
        need_back_shoulder = false;
    }

    // For living streets in Krakow, there aren't separate footways. People can walk in the street.
    // For now, model that by putting shoulders.
    if cfg.inferred_sidewalks || tags.is(osm::HIGHWAY, "living_street") {
        if need_fwd_shoulder {
            fwd_side.push(fwd(LaneType::Shoulder));
        }
        if need_back_shoulder {
            back_side.push(back(LaneType::Shoulder));
        }
    }

    LaneSpec::assemble_ltr(fwd_side, back_side, cfg.driving_side)
}

// See https://wiki.openstreetmap.org/wiki/Proposed_features/cycleway:separation#Typical_values.
// Lots of these mappings are pretty wacky right now. We need more BufferTypes.
#[allow(clippy::ptr_arg)] // Can't chain with `tags.get("foo").and_then` otherwise
fn osm_separation_type(x: &String) -> Option<BufferType> {
    match x.as_ref() {
        "bollard" | "vertical_panel" => Some(BufferType::FlexPosts),
        "kerb" | "separation_kerb" => Some(BufferType::Curb),
        "grass_verge" | "planter" | "tree_row" => Some(BufferType::Planters),
        "guard_rail" | "jersey_barrier" | "railing" => Some(BufferType::JerseyBarrier),
        // TODO Make sure there's a parking lane on that side... also mapped? Any flex posts in
        // between?
        "parking_lane" => None,
        "barred_area" | "dashed_line" | "solid_line" => Some(BufferType::Stripes),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(kv: Vec<&str>) -> Tags {
        let mut tags = Tags::empty();
        for pair in kv {
            let parts = pair.split('=').collect::<Vec<_>>();
            tags.insert(parts[0], parts[1]);
        }
        tags
    }

    #[test]
    fn test_osm_to_specs() {
        let mut ok = true;
        for (url, input, driving_side, expected_lt, expected_dir) in vec![
            (
                "https://www.openstreetmap.org/way/428294122",
                vec![
                    "lanes=2",
                    "oneway=yes",
                    "sidewalk=both",
                    "cycleway:left=lane",
                ],
                DrivingSide::Right,
                "sbdds",
                "v^^^^",
            ),
            (
                "https://www.openstreetmap.org/way/8591383",
                vec![
                    "lanes=1",
                    "oneway=yes",
                    "sidewalk=both",
                    "cycleway:left=track",
                    "oneway:bicycle=no",
                ],
                DrivingSide::Right,
                "sbbds",
                "vv^^^",
            ),
            (
                // A slight variation of the above, using cycleway:left:oneway=no, which should be
                // equivalent
                "https://www.openstreetmap.org/way/8591383",
                vec![
                    "lanes=1",
                    "oneway=yes",
                    "sidewalk=both",
                    "cycleway:left=track",
                    "cycleway:left:oneway=no",
                ],
                DrivingSide::Right,
                "sbbds",
                "vv^^^",
            ),
            (
                "https://www.openstreetmap.org/way/353690151",
                vec![
                    "lanes=4",
                    "sidewalk=both",
                    "parking:lane:both=parallel",
                    "cycleway:right=track",
                    "cycleway:right:oneway=no",
                ],
                DrivingSide::Right,
                "spddddbbps",
                "vvvv^^v^^^",
            ),
            (
                "https://www.openstreetmap.org/way/389654080",
                vec![
                    "lanes=2",
                    "sidewalk=both",
                    "parking:lane:left=parallel",
                    "parking:lane:right=no_stopping",
                    "centre_turn_lane=yes",
                    "cycleway:right=track",
                    "cycleway:right:oneway=no",
                ],
                DrivingSide::Right,
                "spdCdbbs",
                "vvv^^v^^",
            ),
            (
                "https://www.openstreetmap.org/way/369623526",
                vec![
                    "lanes=1",
                    "oneway=yes",
                    "sidewalk=both",
                    "parking:lane:right=diagonal",
                    "cycleway:left=opposite_track",
                    "oneway:bicycle=no",
                ],
                DrivingSide::Right,
                "sbbdps",
                "vv^^^^",
            ),
            (
                "https://www.openstreetmap.org/way/534549104",
                vec![
                    "lanes=2",
                    "oneway=yes",
                    "sidewalk=both",
                    "cycleway:right=track",
                    "cycleway:right:oneway=no",
                    "oneway:bicycle=no",
                ],
                DrivingSide::Right,
                "sddbbs",
                "v^^v^^",
            ),
            (
                "https://www.openstreetmap.org/way/777565028",
                vec!["highway=residential", "oneway=no", "sidewalk=both"],
                DrivingSide::Left,
                "sdds",
                "^^vv",
            ),
            (
                "https://www.openstreetmap.org/way/224637155",
                vec!["lanes=2", "oneway=yes", "sidewalk=left"],
                DrivingSide::Left,
                "sdd",
                "^^^",
            ),
            (
                "https://www.openstreetmap.org/way/4188078",
                vec![
                    "lanes=2",
                    "cycleway:left=lane",
                    "oneway=yes",
                    "sidewalk=left",
                ],
                DrivingSide::Left,
                "sbdd",
                "^^^^",
            ),
            (
                "https://www.openstreetmap.org/way/49207928",
                vec!["cycleway:right=lane", "sidewalk=both"],
                DrivingSide::Left,
                "sddbs",
                "^^vvv",
            ),
            // How should an odd number of lanes forward/backwards be split without any clues?
            (
                "https://www.openstreetmap.org/way/898731283",
                vec!["lanes=3", "sidewalk=both"],
                DrivingSide::Left,
                "sddds",
                "^^^vv",
            ),
            (
                // I didn't look for a real example of this
                "https://www.openstreetmap.org/way/898731283",
                vec!["lanes=5", "sidewalk=none"],
                DrivingSide::Right,
                "SdddddS",
                "vvv^^^^",
            ),
            (
                "https://www.openstreetmap.org/way/335668924",
                vec!["lanes=1", "sidewalk=none"],
                DrivingSide::Right,
                "SddS",
                "vv^^",
            ),
        ] {
            let cfg = MapConfig {
                driving_side,
                bikes_can_use_bus_lanes: true,
                inferred_sidewalks: true,
                street_parking_spot_length: geom::Distance::meters(8.0),
                turn_on_red: true,
            };
            let actual = get_lane_specs_ltr(&tags(input.clone()), &cfg);
            let actual_lt: String = actual.iter().map(|s| s.lt.to_char()).collect();
            let actual_dir: String = actual
                .iter()
                .map(|s| if s.dir == Direction::Fwd { '^' } else { 'v' })
                .collect();
            if actual_lt != expected_lt || actual_dir != expected_dir {
                ok = false;
                println!("For input (example from {}):", url);
                for kv in input {
                    println!("    {}", kv);
                }
                println!("Got:");
                println!("    {}", actual_lt);
                println!("    {}", actual_dir);
                println!("Expected:");
                println!("    {}", expected_lt);
                println!("    {}", expected_dir);
                println!();
            }
        }
        assert!(ok);
    }
}

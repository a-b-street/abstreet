// Purely from OSM tags, determine the lanes that a road segment has.

use std::iter;

use abstutil::Tags;
use geom::Distance;

use crate::{
    osm, Direction, DrivingSide, LaneType, NORMAL_LANE_THICKNESS, SERVICE_ROAD_LANE_THICKNESS,
    SHOULDER_THICKNESS, SIDEWALK_THICKNESS,
};

#[derive(PartialEq)]
pub struct LaneSpec {
    pub lt: LaneType,
    pub dir: Direction,
    pub width: Distance,
}

fn fwd(lt: LaneType) -> LaneSpec {
    LaneSpec {
        lt,
        dir: Direction::Fwd,
        width: match lt {
            LaneType::Sidewalk => SIDEWALK_THICKNESS,
            LaneType::Shoulder => SHOULDER_THICKNESS,
            _ => NORMAL_LANE_THICKNESS,
        },
    }
}

fn back(lt: LaneType) -> LaneSpec {
    LaneSpec {
        lt,
        dir: Direction::Back,
        width: match lt {
            LaneType::Sidewalk => SIDEWALK_THICKNESS,
            LaneType::Shoulder => SHOULDER_THICKNESS,
            _ => NORMAL_LANE_THICKNESS,
        },
    }
}

pub fn get_lane_specs_ltr(tags: &Tags, driving_side: DrivingSide) -> Vec<LaneSpec> {
    // Easy special cases first.
    if tags.is_any("railway", vec!["light_rail", "rail"]) {
        return vec![fwd(LaneType::LightRail)];
    }
    if tags.is(osm::HIGHWAY, "footway") {
        return vec![fwd(LaneType::Sidewalk)];
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
            // TODO Really, this is ambiguous, but...
            (n / 2).max(1)
        }
    } else {
        // TODO Grrr.
        1
    };
    let num_driving_back = if let Some(n) = tags
        .get("lanes:backward")
        .and_then(|num| num.parse::<usize>().ok())
    {
        n
    } else if let Some(n) = tags.get("lanes").and_then(|num| num.parse::<usize>().ok()) {
        if oneway {
            0
        } else if n % 2 == 0 {
            n / 2
        } else {
            // TODO Really, this is ambiguous, but...
            (n / 2).max(1)
        }
    } else {
        // TODO Grrr.
        if oneway {
            0
        } else {
            1
        }
    };

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
        return assemble_ltr(fwd_side, back_side, driving_side);
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
        let parts: Vec<&str> = fwd_bus_spec.split("|").collect();
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
        let parts: Vec<&str> = spec.split("|").collect();
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
        if tags.is_any("cycleway:right", vec!["lane", "track"]) {
            if tags.is("cycleway:right:oneway", "no") || tags.is("oneway:bicycle", "no") {
                fwd_side.push(back(LaneType::Biking));
            }
            fwd_side.push(fwd(LaneType::Biking));
        }
        if tags.is("cycleway:left", "opposite_lane") || tags.is("cycleway", "opposite_lane") {
            back_side.push(back(LaneType::Biking));
        }
        if tags.is_any("cycleway:left", vec!["lane", "opposite_track", "track"]) {
            if oneway {
                fwd_side.insert(0, fwd(LaneType::Biking));
                if tags.is("oneway:bicycle", "no") {
                    back_side.push(back(LaneType::Biking));
                }
            } else {
                back_side.push(back(LaneType::Biking));
            }
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
    } else if tags.is(osm::SIDEWALK, "separate") {
        // TODO Need to snap separate sidewalks to ways. Until then, just do this.
        fwd_side.push(fwd(LaneType::Sidewalk));
        if !back_side.is_empty() {
            back_side.push(back(LaneType::Sidewalk));
        }
    } else if tags.is(osm::SIDEWALK, "right") {
        if driving_side == DrivingSide::Right {
            fwd_side.push(fwd(LaneType::Sidewalk));
        } else {
            back_side.push(back(LaneType::Sidewalk));
        }
    } else if tags.is(osm::SIDEWALK, "left") {
        if driving_side == DrivingSide::Right {
            back_side.push(back(LaneType::Sidewalk));
        } else {
            fwd_side.push(fwd(LaneType::Sidewalk));
        }
    }

    if tags.is(osm::HIGHWAY, "service") || tags.is("narrow", "yes") {
        for spec in fwd_side.iter_mut().chain(back_side.iter_mut()) {
            if spec.lt == LaneType::Driving || spec.lt == LaneType::Parking {
                spec.width = SERVICE_ROAD_LANE_THICKNESS;
            }
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

    if need_fwd_shoulder {
        fwd_side.push(fwd(LaneType::Shoulder));
    }
    if need_back_shoulder {
        back_side.push(back(LaneType::Shoulder));
    }

    assemble_ltr(fwd_side, back_side, driving_side)
}

fn assemble_ltr(
    mut fwd_side: Vec<LaneSpec>,
    mut back_side: Vec<LaneSpec>,
    driving_side: DrivingSide,
) -> Vec<LaneSpec> {
    match driving_side {
        DrivingSide::Right => {
            back_side.reverse();
            back_side.extend(fwd_side);
            back_side
        }
        DrivingSide::Left => {
            fwd_side.reverse();
            fwd_side.extend(back_side);
            fwd_side
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lt_to_char(lt: LaneType) -> &'static str {
        match lt {
            LaneType::Driving => "d",
            LaneType::Biking => "b",
            LaneType::Bus => "B",
            LaneType::Parking => "p",
            LaneType::Sidewalk => "s",
            LaneType::Shoulder => "S",
            LaneType::SharedLeftTurn => "C",
            LaneType::Construction => "x",
            LaneType::LightRail => "l",
        }
    }

    fn tags(kv: Vec<&str>) -> Tags {
        let mut tags = Tags::new(std::collections::BTreeMap::new());
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
        ] {
            let actual = get_lane_specs_ltr(&tags(input.clone()), driving_side);
            let actual_lt = actual
                .iter()
                .map(|s| lt_to_char(s.lt))
                .collect::<Vec<_>>()
                .join("");
            let actual_dir = actual
                .iter()
                .map(|s| if s.dir == Direction::Fwd { "^" } else { "v" })
                .collect::<Vec<_>>()
                .join("");
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
                println!("");
            }
        }
        assert!(ok);
    }
}

use crate::{osm, LaneType, NORMAL_LANE_THICKNESS, SHOULDER_THICKNESS, SIDEWALK_THICKNESS};
use abstutil::Tags;
use geom::Distance;
use std::iter;

pub struct LaneSpec {
    pub lane_type: LaneType,
    pub reverse_pts: bool,
    pub width: Distance,
}

impl LaneSpec {
    pub fn fwds(lane_type: LaneType) -> LaneSpec {
        LaneSpec {
            lane_type,
            reverse_pts: false,
            width: if lane_type == LaneType::Sidewalk {
                SIDEWALK_THICKNESS
            } else {
                NORMAL_LANE_THICKNESS
            },
        }
    }

    pub fn back(lane_type: LaneType) -> LaneSpec {
        LaneSpec {
            lane_type,
            reverse_pts: true,
            width: if lane_type == LaneType::Sidewalk {
                SIDEWALK_THICKNESS
            } else {
                NORMAL_LANE_THICKNESS
            },
        }
    }

    fn normal(fwd: Vec<LaneType>, back: Vec<LaneType>) -> Vec<LaneSpec> {
        let mut specs: Vec<LaneSpec> = Vec::new();
        for lt in back.into_iter().rev() {
            specs.push(LaneSpec::back(lt));
        }
        for lt in fwd {
            specs.push(LaneSpec::fwds(lt));
        }
        assert!(!specs.is_empty());
        specs
    }
}

// TODO This is ripe for unit testing.
pub fn get_lane_specs_ltr(tags: &Tags) -> Vec<LaneSpec> {
    // Easy special cases first.
    if tags.is_any("railway", vec!["light_rail", "rail"]) {
        return LaneSpec::normal(vec![LaneType::LightRail], Vec::new());
    }
    if tags.is(osm::HIGHWAY, "footway") {
        return LaneSpec::normal(vec![LaneType::Sidewalk], Vec::new());
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

    let mut fwd_side: Vec<LaneType> = iter::repeat(driving_lane).take(num_driving_fwd).collect();
    let mut back_side: Vec<LaneType> = iter::repeat(driving_lane).take(num_driving_back).collect();
    // TODO Fix upstream. https://wiki.openstreetmap.org/wiki/Key:centre_turn_lane
    if tags.is("lanes:both_ways", "1") || tags.is("centre_turn_lane", "yes") {
        fwd_side.insert(0, LaneType::SharedLeftTurn);
    }

    if driving_lane == LaneType::Construction {
        return LaneSpec::normal(fwd_side, back_side);
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
    {
        let parts: Vec<&str> = fwd_bus_spec.split("|").collect();
        let offset = if fwd_side[0] == LaneType::SharedLeftTurn {
            1
        } else {
            0
        };
        if parts.len() == fwd_side.len() - offset {
            for (idx, part) in parts.into_iter().enumerate() {
                if part == "designated" {
                    fwd_side[idx + offset] = LaneType::Bus;
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
                    back_side[idx] = LaneType::Bus;
                }
            }
        }
    }

    if tags.is_any("cycleway", vec!["lane", "track"]) {
        fwd_side.push(LaneType::Biking);
        if !back_side.is_empty() {
            back_side.push(LaneType::Biking);
        }
    } else if tags.is_any("cycleway:both", vec!["lane", "track"]) {
        fwd_side.push(LaneType::Biking);
        back_side.push(LaneType::Biking);
    } else {
        if tags.is_any("cycleway:right", vec!["lane", "track"]) {
            fwd_side.push(LaneType::Biking);
        }
        if tags.is("cycleway:left", "opposite_lane") || tags.is("cycleway", "opposite_lane") {
            back_side.push(LaneType::Biking);
        }
        if tags.is_any("cycleway:left", vec!["lane", "track"]) {
            if oneway {
                fwd_side.insert(0, LaneType::Biking);
            } else {
                back_side.push(LaneType::Biking);
            }
        }

        // Cycleway isn't explicitly specified, but this is a reasonable assumption anyway.
        if back_side.is_empty() && tags.is("oneway:bicycle", "no") {
            back_side.push(LaneType::Biking);
        }
    }

    if driving_lane == LaneType::Driving {
        let has_parking = vec!["parallel", "diagonal", "perpendicular"];
        let parking_lane_fwd = tags.is_any(osm::PARKING_RIGHT, has_parking.clone())
            || tags.is_any(osm::PARKING_BOTH, has_parking.clone());
        let parking_lane_back = tags.is_any(osm::PARKING_LEFT, has_parking.clone())
            || tags.is_any(osm::PARKING_BOTH, has_parking);
        if parking_lane_fwd {
            fwd_side.push(LaneType::Parking);
        }
        if parking_lane_back {
            back_side.push(LaneType::Parking);
        }
    }

    if tags.is(osm::SIDEWALK, "both") {
        fwd_side.push(LaneType::Sidewalk);
        back_side.push(LaneType::Sidewalk);
    } else if tags.is(osm::SIDEWALK, "separate") {
        // TODO Need to snap separate sidewalks to ways. Until then, just do this.
        fwd_side.push(LaneType::Sidewalk);
        if !back_side.is_empty() {
            back_side.push(LaneType::Sidewalk);
        }
    } else if tags.is(osm::SIDEWALK, "right") {
        fwd_side.push(LaneType::Sidewalk);
    } else if tags.is(osm::SIDEWALK, "left") {
        back_side.push(LaneType::Sidewalk);
    }

    let mut need_fwd_shoulder = fwd_side
        .last()
        .map(|lt| *lt != LaneType::Sidewalk)
        .unwrap_or(true);
    let mut need_back_shoulder = back_side
        .last()
        .map(|lt| *lt != LaneType::Sidewalk)
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

    let mut specs = LaneSpec::normal(fwd_side, back_side);
    if need_fwd_shoulder {
        specs.push(LaneSpec {
            lane_type: LaneType::Shoulder,
            reverse_pts: false,
            width: SHOULDER_THICKNESS,
        });
    }
    if need_back_shoulder {
        specs.insert(
            0,
            LaneSpec {
                lane_type: LaneType::Shoulder,
                reverse_pts: true,
                width: SHOULDER_THICKNESS,
            },
        );
    }

    specs
}

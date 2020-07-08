use crate::{osm, LaneType};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{fmt, iter};

// (original direction, reversed direction)
pub fn get_lane_types(osm_tags: &BTreeMap<String, String>) -> (Vec<LaneType>, Vec<LaneType>) {
    let tags = Tags(osm_tags);

    if let Some(s) = osm_tags.get(osm::SYNTHETIC_LANES) {
        if let Some(spec) = RoadSpec::parse(s.to_string()) {
            return (spec.fwd, spec.back);
        } else {
            panic!("Bad {} RoadSpec: {}", osm::SYNTHETIC_LANES, s);
        }
    }

    // Easy special cases first.
    if tags.is("railway", "light_rail") {
        return (vec![LaneType::LightRail], Vec::new());
    }
    if tags.is(osm::HIGHWAY, "footway") {
        return (vec![LaneType::Sidewalk], Vec::new());
    }

    // TODO Reversible roads should be handled differently?
    let oneway = tags.is("oneway", "yes")
        || tags.is("oneway", "reversible")
        || tags.is("junction", "roundabout");

    // How many driving lanes in each direction?
    let num_driving_fwd = if let Some(n) = osm_tags
        .get("lanes:forward")
        .and_then(|num| num.parse::<usize>().ok())
    {
        n
    } else if let Some(n) = osm_tags
        .get("lanes")
        .and_then(|num| num.parse::<usize>().ok())
    {
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
    let num_driving_back = if let Some(n) = osm_tags
        .get("lanes:backward")
        .and_then(|num| num.parse::<usize>().ok())
    {
        n
    } else if let Some(n) = osm_tags
        .get("lanes")
        .and_then(|num| num.parse::<usize>().ok())
    {
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

    // Sup West Seattle
    let driving_lane = if tags.is("access", "no") && tags.is("bus", "yes") {
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
        return (fwd_side, back_side);
    }

    // TODO Handle bus lanes properly.
    let has_bus_lane = osm_tags.contains_key("bus:lanes");
    if has_bus_lane {
        fwd_side.pop();
        fwd_side.push(LaneType::Bus);
        if !back_side.is_empty() {
            back_side.pop();
            back_side.push(LaneType::Bus);
        }
    }

    if tags.is("cycleway", "lane") {
        fwd_side.push(LaneType::Biking);
        if !back_side.is_empty() {
            back_side.push(LaneType::Biking);
        }
    } else {
        if tags.is("cycleway:right", "lane") {
            fwd_side.push(LaneType::Biking);
        }
        if tags.is("cycleway:left", "lane")
            || tags.is("cycleway:left", "opposite_lane")
            || tags.is("cycleway", "opposite_lane")
        {
            back_side.push(LaneType::Biking);
        }

        // Cycleway isn't explicitly specified, but this is a reasonable assumption anyway.
        if back_side.is_empty() && tags.is("oneway:bicycle", "no") {
            back_side.push(LaneType::Biking);
        }
    }

    if driving_lane == LaneType::Driving {
        fn has_parking(value: Option<&String>) -> bool {
            value == Some(&"parallel".to_string())
                || value == Some(&"diagonal".to_string())
                || value == Some(&"perpendicular".to_string())
        }
        let parking_lane_fwd = has_parking(osm_tags.get(osm::PARKING_RIGHT))
            || has_parking(osm_tags.get(osm::PARKING_BOTH));
        let parking_lane_back = has_parking(osm_tags.get(osm::PARKING_LEFT))
            || has_parking(osm_tags.get(osm::PARKING_BOTH));
        if parking_lane_fwd {
            fwd_side.push(LaneType::Parking);
        }
        if parking_lane_back {
            back_side.push(LaneType::Parking);
        }
    }

    // TODO Need to snap separate sidewalks to ways. Until then, just do this.
    if tags.is(osm::SIDEWALK, "both") || tags.is(osm::SIDEWALK, "separate") {
        fwd_side.push(LaneType::Sidewalk);
        back_side.push(LaneType::Sidewalk);
    } else if tags.is(osm::SIDEWALK, "right") {
        fwd_side.push(LaneType::Sidewalk);
    } else if tags.is(osm::SIDEWALK, "left") {
        back_side.push(LaneType::Sidewalk);
    }

    (fwd_side, back_side)
}

// TODO Figure out Rust strings; there's maybe a less verbose way than the old
// osm_tags.get("cycleway:right") == Some(&"lane".to_string()) mess.
struct Tags<'a>(&'a BTreeMap<String, String>);
impl<'a> Tags<'a> {
    fn is(&self, k: &str, v: &str) -> bool {
        self.0.get(k) == Some(&v.to_string())
    }
}

// This is a convenient way for map_editor to plumb instructions here.
#[derive(Serialize, Deserialize)]
pub struct RoadSpec {
    pub fwd: Vec<LaneType>,
    pub back: Vec<LaneType>,
}

impl fmt::Display for RoadSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for lt in &self.fwd {
            write!(f, "{}", RoadSpec::lt_to_char(*lt))?;
        }
        write!(f, "/")?;
        for lt in &self.back {
            write!(f, "{}", RoadSpec::lt_to_char(*lt))?;
        }
        Ok(())
    }
}

impl RoadSpec {
    pub fn parse(s: String) -> Option<RoadSpec> {
        let mut fwd: Vec<LaneType> = Vec::new();
        let mut back: Vec<LaneType> = Vec::new();
        let mut seen_slash = false;
        for c in s.chars() {
            if !seen_slash && c == '/' {
                seen_slash = true;
            } else if let Some(lt) = RoadSpec::char_to_lt(c) {
                if seen_slash {
                    back.push(lt);
                } else {
                    fwd.push(lt);
                }
            } else {
                return None;
            }
        }
        if seen_slash && (fwd.len() + back.len()) > 0 {
            Some(RoadSpec { fwd, back })
        } else {
            None
        }
    }

    fn lt_to_char(lt: LaneType) -> char {
        match lt {
            LaneType::Driving => 'd',
            LaneType::Parking => 'p',
            LaneType::Sidewalk => 's',
            LaneType::Biking => 'b',
            LaneType::Bus => 'u',
            LaneType::SharedLeftTurn => 'l',
            LaneType::Construction => 'c',
            LaneType::LightRail => 'r',
        }
    }

    fn char_to_lt(c: char) -> Option<LaneType> {
        match c {
            'd' => Some(LaneType::Driving),
            'p' => Some(LaneType::Parking),
            's' => Some(LaneType::Sidewalk),
            'b' => Some(LaneType::Biking),
            'u' => Some(LaneType::Bus),
            'l' => Some(LaneType::SharedLeftTurn),
            'c' => Some(LaneType::Construction),
            'r' => Some(LaneType::LightRail),
            _ => None,
        }
    }
}

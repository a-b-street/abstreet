use crate::{raw_data, LaneType, MapEdits, RoadID};
use serde_derive::{Deserialize, Serialize};
use std::iter;

// (original direction, reversed direction)
fn get_lanes(r: &raw_data::Road) -> (Vec<LaneType>, Vec<LaneType>) {
    // The raw_data might come from the synthetic map editor.
    if let Some(s) = r.osm_tags.get("synthetic_lanes") {
        if let Some(spec) = RoadSpec::parse(s.to_string()) {
            return (spec.fwd, spec.back);
        } else {
            panic!("Bad synthetic_lanes RoadSpec: {}", s);
        }
    }

    // Easy special cases first.
    if r.osm_tags.get("junction") == Some(&"roundabout".to_string()) {
        return (vec![LaneType::Driving, LaneType::Sidewalk], Vec::new());
    }
    if r.osm_tags.get("highway") == Some(&"footway".to_string()) {
        return (vec![LaneType::Sidewalk], Vec::new());
    }

    // TODO Reversible roads should be handled differently?
    let oneway = r.osm_tags.get("oneway") == Some(&"yes".to_string())
        || r.osm_tags.get("oneway") == Some(&"reversible".to_string());
    let num_driving_lanes_per_road = if let Some(n) = r
        .osm_tags
        .get("lanes")
        .and_then(|num| num.parse::<usize>().ok())
    {
        n
    } else {
        // TODO https://wiki.openstreetmap.org/wiki/Key:lanes#Assumptions service, track, and path
        // should have less, but I don't see examples of these
        2
    };
    let mut driving_lanes_per_side: Vec<LaneType> = iter::repeat(LaneType::Driving)
        .take(if oneway {
            num_driving_lanes_per_road
        } else {
            // TODO OSM way 124940792 is I5 express lane, should it be considered oneway?
            (num_driving_lanes_per_road / 2).max(1)
        })
        .collect();
    // TODO Don't even bother trying to parse this yet.
    let has_bus_lane = r.osm_tags.contains_key("bus:lanes");
    // TODO This is circumstantial at best. :)
    if has_bus_lane && driving_lanes_per_side.len() > 1 {
        driving_lanes_per_side.pop();
    }

    let has_bike_lane = r.osm_tags.get("cycleway") == Some(&"lane".to_string());
    let has_sidewalk = r.osm_tags.get("highway") != Some(&"motorway".to_string())
        && r.osm_tags.get("highway") != Some(&"motorway_link".to_string());
    // TODO Bus/bike and parking lanes can coexist, but then we have to make sure cars are fine
    // with merging in/out of the bus/bike lane to park. ><
    //let has_parking = has_sidewalk && !has_bus_lane && !has_bike_lane;

    let mut fwd_side = driving_lanes_per_side.clone();
    if has_bus_lane {
        fwd_side.push(LaneType::Bus);
    }
    if has_bike_lane {
        fwd_side.push(LaneType::Biking);
    }
    if r.parking_lane_fwd {
        fwd_side.push(LaneType::Parking);
    }
    if has_sidewalk {
        fwd_side.push(LaneType::Sidewalk);
    }

    if oneway {
        // Only residential streets have a sidewalk on the other side of a one-way.
        // Ignore off-side parking, since cars don't know how to park on lanes without a driving
        // lane in that direction too.
        let back_side =
            if has_sidewalk && r.osm_tags.get("highway") == Some(&"residential".to_string()) {
                vec![LaneType::Sidewalk]
            } else {
                Vec::new()
            };
        (fwd_side, back_side)
    } else {
        let mut back_side = driving_lanes_per_side;
        if has_bus_lane {
            back_side.push(LaneType::Bus);
        }
        if has_bike_lane {
            back_side.push(LaneType::Biking);
        }
        if r.parking_lane_back {
            back_side.push(LaneType::Parking);
        }
        if has_sidewalk {
            back_side.push(LaneType::Sidewalk);
        }
        (fwd_side, back_side)
    }
}

#[derive(Debug, PartialEq)]
pub struct LaneSpec {
    pub lane_type: LaneType,
    pub offset: u8,
    pub reverse_pts: bool,
}

impl LaneSpec {
    fn new(lane_type: LaneType, offset: u8, reverse_pts: bool) -> LaneSpec {
        LaneSpec {
            lane_type,
            offset,
            reverse_pts,
        }
    }
}

pub fn get_lane_specs(r: &raw_data::Road, id: RoadID, edits: &MapEdits) -> Vec<LaneSpec> {
    let (side1_types, side2_types) = if let Some(e) = edits.roads.get(&id) {
        info!("Using edits for {}", id);
        (e.forwards_lanes.clone(), e.backwards_lanes.clone())
    } else {
        get_lanes(r)
    };

    let mut specs: Vec<LaneSpec> = Vec::new();
    for (idx, lane_type) in side1_types.iter().enumerate() {
        specs.push(LaneSpec::new(*lane_type, idx as u8, false));
    }
    for (idx, lane_type) in side2_types.iter().enumerate() {
        specs.push(LaneSpec::new(*lane_type, idx as u8, true));
    }
    if specs.is_empty() {
        panic!("{} wound up with no lanes! {:?}", id, r);
    }
    specs
}

// This is a convenient way for the synthetic map editor to plumb instructions here.
#[derive(Serialize, Deserialize)]
pub struct RoadSpec {
    pub fwd: Vec<LaneType>,
    pub back: Vec<LaneType>,
}

impl RoadSpec {
    pub fn to_string(&self) -> String {
        let mut s: Vec<char> = Vec::new();
        for lt in &self.fwd {
            s.push(RoadSpec::lt_to_char(*lt));
        }
        s.push('/');
        for lt in &self.back {
            s.push(RoadSpec::lt_to_char(*lt));
        }
        s.into_iter().collect()
    }

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
        }
    }

    fn char_to_lt(c: char) -> Option<LaneType> {
        match c {
            'd' => Some(LaneType::Driving),
            'p' => Some(LaneType::Parking),
            's' => Some(LaneType::Sidewalk),
            'b' => Some(LaneType::Biking),
            'u' => Some(LaneType::Bus),
            _ => None,
        }
    }
}

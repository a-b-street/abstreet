use edits::Edits;
use lane::LaneType;
use raw_data;
use road::RoadID;
use std::iter;

// (original direction, reversed direction)
fn get_lanes(r: &raw_data::Road) -> (Vec<LaneType>, Vec<LaneType>) {
    let oneway = r.osm_tags.get("oneway") == Some(&"yes".to_string());
    // These seem to represent weird roundabouts
    let junction = r.osm_tags.get("junction") == Some(&"yes".to_string());
    let big_highway = r.osm_tags.get("highway") == Some(&"motorway".to_string());
    let bike_lane = r.osm_tags.get("cycleway") == Some(&"lane".to_string());
    let num_driving_lanes = if let Some(n) = r.osm_tags
        .get("lanes")
        .and_then(|num| num.parse::<usize>().ok())
    {
        n
    } else if r.osm_tags.get("highway") == Some(&"primary".to_string())
        || r.osm_tags.get("highway") == Some(&"secondary".to_string())
    {
        2
    } else {
        1
    };

    if junction {
        return (vec![LaneType::Driving], Vec::new());
    }

    // The lanes tag is of course ambiguous, but seems to usually mean total number of lanes for
    // both directions of the road.
    let driving_lanes: Vec<LaneType> = iter::repeat(LaneType::Driving)
        .take(num_driving_lanes / 2)
        .collect();
    if big_highway {
        if oneway {
            let mut all_lanes = Vec::new();
            all_lanes.extend(driving_lanes.clone());
            all_lanes.extend(driving_lanes);
            return (all_lanes, Vec::new());
        } else {
            return (driving_lanes.clone(), driving_lanes);
        }
    }

    let mut full_side = driving_lanes;
    if bike_lane {
        full_side.push(LaneType::Biking);
    }
    full_side.push(LaneType::Parking);
    full_side.push(LaneType::Sidewalk);
    if oneway {
        (full_side, vec![LaneType::Sidewalk])
    } else {
        (full_side.clone(), full_side)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct LaneSpec {
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

pub(crate) fn get_lane_specs(r: &raw_data::Road, id: RoadID, edits: &Edits) -> Vec<LaneSpec> {
    let (side1_types, side2_types) = if let Some(e) = edits.roads.get(&id) {
        println!("Using edits for {}", id);
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
    specs
}

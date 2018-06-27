use raw_data;
use road::LaneType;
use std::iter;

// (original direction, reversed direction)
fn get_lanes(r: &raw_data::Road) -> (Vec<LaneType>, Vec<LaneType>) {
    let oneway = r.osm_tags.get("oneway") == Some(&"yes".to_string());
    // These seem to represent weird roundabouts
    let junction = r.osm_tags.get("junction") == Some(&"yes".to_string());
    let big_road = r.osm_tags.get("highway") == Some(&"primary".to_string())
        || r.osm_tags.get("highway") == Some(&"secondary".to_string());
    // TODO debugging convenience
    let only_roads_for_debugging = false;

    if junction {
        return (vec![LaneType::Driving], Vec::new());
    }

    let num_driving_lanes = if big_road { 2 } else { 1 };
    let driving_lanes: Vec<LaneType> = iter::repeat(LaneType::Driving)
        .take(num_driving_lanes)
        .collect();
    if only_roads_for_debugging {
        if oneway {
            return (driving_lanes, Vec::new());
        } else {
            return (driving_lanes.clone(), driving_lanes);
        }
    }

    let mut full_side = driving_lanes;
    full_side.push(LaneType::Parking);
    full_side.push(LaneType::Sidewalk);
    if oneway {
        (full_side, vec![LaneType::Sidewalk])
    } else {
        (full_side.clone(), full_side)
    }
}

pub(crate) struct LaneSpec {
    pub lane_type: LaneType,
    pub offset: u8,
    pub reverse_pts: bool,
    pub offset_for_other_id: Option<isize>,
}

pub(crate) fn get_lane_specs(r: &raw_data::Road) -> Vec<LaneSpec> {
    let mut specs: Vec<LaneSpec> = Vec::new();

    let (side1_types, side2_types) = get_lanes(r);
    for (idx, lane_type) in side1_types.iter().enumerate() {
        // TODO this might be a bit wrong. add unit tests. :)
        let offset_for_other_id = if *lane_type != LaneType::Driving {
            None
        } else if !side2_types.contains(&LaneType::Driving) {
            None
        } else if side1_types == side2_types {
            Some(side1_types.len() as isize)
        } else {
            panic!("get_lane_specs case not handled yet");
        };

        specs.push(LaneSpec {
            offset_for_other_id,
            lane_type: *lane_type,
            offset: idx as u8,
            reverse_pts: false,
        });
    }
    for (idx, lane_type) in side2_types.iter().enumerate() {
        let offset_for_other_id = if *lane_type != LaneType::Driving {
            None
        } else {
            Some(-1 * (side1_types.len() as isize))
        };

        specs.push(LaneSpec {
            offset_for_other_id,
            lane_type: *lane_type,
            offset: idx as u8,
            reverse_pts: true,
        });
    }

    specs
}

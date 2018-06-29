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

#[derive(Debug, PartialEq)]
pub(crate) struct LaneSpec {
    pub lane_type: LaneType,
    pub offset: u8,
    pub reverse_pts: bool,
    pub offset_for_other_id: Option<isize>,
}

impl LaneSpec {
    fn new(
        lane_type: LaneType,
        offset: u8,
        reverse_pts: bool,
        offset_for_other_id: Option<isize>,
    ) -> LaneSpec {
        LaneSpec {
            lane_type,
            offset,
            reverse_pts,
            offset_for_other_id,
        }
    }
}

pub(crate) fn get_lane_specs(r: &raw_data::Road) -> Vec<LaneSpec> {
    lane_specs_for(get_lanes(r))
}

fn lane_specs_for((side1_types, side2_types): (Vec<LaneType>, Vec<LaneType>)) -> Vec<LaneSpec> {
    let mut specs: Vec<LaneSpec> = Vec::new();

    // This seems like a messy approach. :\
    let sidewalk1_idx = side1_types.iter().position(|&lt| lt == LaneType::Sidewalk);
    let sidewalk2_idx = side2_types.iter().position(|&lt| lt == LaneType::Sidewalk);

    for (idx, lane_type) in side1_types.iter().enumerate() {
        let offset_for_other_id = match lane_type {
            LaneType::Sidewalk => {
                sidewalk2_idx.map(|idx| (side1_types.len() - sidewalk1_idx.unwrap() + idx) as isize)
            }
            LaneType::Parking => None,
            LaneType::Driving => if !side2_types.contains(&LaneType::Driving) {
                None
            } else {
                assert!(side1_types == side2_types);
                Some(side1_types.len() as isize)
            },
        };

        specs.push(LaneSpec::new(
            *lane_type,
            idx as u8,
            false,
            offset_for_other_id,
        ));
    }
    for (idx, lane_type) in side2_types.iter().enumerate() {
        let offset_for_other_id = match lane_type {
            LaneType::Parking => None,
            LaneType::Sidewalk => sidewalk2_idx
                .map(|idx| -1 * ((side1_types.len() - sidewalk1_idx.unwrap() + idx) as isize)),
            LaneType::Driving => Some(-1 * (side1_types.len() as isize)),
        };

        specs.push(LaneSpec::new(
            *lane_type,
            idx as u8,
            true,
            offset_for_other_id,
        ));
    }

    specs
}

#[test]
fn junction() {
    let d = LaneType::Driving;

    assert_eq!(
        lane_specs_for((vec![d], vec![])),
        vec![LaneSpec::new(d, 0, false, None)]
    );
}

#[test]
fn oneway() {
    let d = LaneType::Driving;
    let p = LaneType::Parking;
    let s = LaneType::Sidewalk;

    assert_eq!(
        lane_specs_for((vec![d, p, s], vec![s])),
        vec![
            LaneSpec::new(d, 0, false, None),
            LaneSpec::new(p, 1, false, None),
            LaneSpec::new(s, 2, false, Some(1)),
            LaneSpec::new(s, 0, true, Some(-1)),
        ]
    );
}

#[test]
fn twoway() {
    let d = LaneType::Driving;
    let p = LaneType::Parking;
    let s = LaneType::Sidewalk;

    assert_eq!(
        lane_specs_for((vec![d, p, s], vec![d, p, s])),
        vec![
            LaneSpec::new(d, 0, false, Some(3)),
            LaneSpec::new(p, 1, false, None),
            LaneSpec::new(s, 2, false, Some(3)),
            LaneSpec::new(d, 0, true, Some(-3)),
            LaneSpec::new(p, 1, true, None),
            LaneSpec::new(s, 2, true, Some(-3)),
        ]
    );
}

#[test]
fn big_twoway() {
    let d = LaneType::Driving;
    let p = LaneType::Parking;
    let s = LaneType::Sidewalk;

    assert_eq!(
        lane_specs_for((vec![d, d, p, s], vec![d, d, p, s])),
        vec![
            LaneSpec::new(d, 0, false, Some(4)),
            LaneSpec::new(d, 1, false, Some(4)),
            LaneSpec::new(p, 2, false, None),
            LaneSpec::new(s, 3, false, Some(4)),
            LaneSpec::new(d, 0, true, Some(-4)),
            LaneSpec::new(d, 1, true, Some(-4)),
            LaneSpec::new(p, 2, true, None),
            LaneSpec::new(s, 3, true, Some(-4)),
        ]
    );
}

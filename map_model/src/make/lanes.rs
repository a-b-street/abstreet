use raw_data;
use road::LaneType;
use std::iter;

// (original direction, reversed direction)
fn get_lanes(r: &raw_data::Road) -> (Vec<LaneType>, Vec<LaneType>) {
    let oneway = r.osm_tags.get("oneway") == Some(&"yes".to_string());
    // These seem to represent weird roundabouts
    let junction = r.osm_tags.get("junction") == Some(&"yes".to_string());
    let big_highway = r.osm_tags.get("highway") == Some(&"motorway".to_string());
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
    // TODO debugging convenience
    let only_roads_for_debugging = false;

    if junction {
        return (vec![LaneType::Driving], Vec::new());
    }

    // The lanes tag is of course ambiguous, but seems to usually mean total number of lanes for
    // both directions of the road.
    let driving_lanes: Vec<LaneType> = iter::repeat(LaneType::Driving)
        .take(num_driving_lanes / 2)
        .collect();
    if only_roads_for_debugging || big_highway {
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
    // TODO have a better idea where bike lanes are
    if r.osm_way_id % 10 == 0 {
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
    pub offset_for_other_id: Option<isize>,
    pub offsets_for_siblings: Vec<isize>,
}

impl LaneSpec {
    fn new(
        lane_type: LaneType,
        offset: u8,
        reverse_pts: bool,
        offset_for_other_id: Option<isize>,
        offsets_for_siblings: Vec<isize>,
    ) -> LaneSpec {
        LaneSpec {
            lane_type,
            offset,
            reverse_pts,
            offset_for_other_id,
            offsets_for_siblings,
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
            LaneType::Biking => if !side2_types.contains(&LaneType::Biking) {
                None
            } else {
                assert!(side1_types == side2_types);
                Some(side1_types.len() as isize)
            },
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
            make_offsets(idx as isize, side1_types.len() as isize),
        ));
    }
    for (idx, lane_type) in side2_types.iter().enumerate() {
        let offset_for_other_id = match lane_type {
            LaneType::Parking => None,
            LaneType::Biking => Some(-1 * (side1_types.len() as isize)),
            LaneType::Sidewalk => sidewalk2_idx
                .map(|idx| -1 * ((side1_types.len() - sidewalk1_idx.unwrap() + idx) as isize)),
            LaneType::Driving => Some(-1 * (side1_types.len() as isize)),
        };

        specs.push(LaneSpec::new(
            *lane_type,
            idx as u8,
            true,
            offset_for_other_id,
            make_offsets(idx as isize, side2_types.len() as isize),
        ));
    }

    specs
}

fn make_offsets(idx: isize, len: isize) -> Vec<isize> {
    let mut offsets = Vec::new();
    for i in 0..len {
        if i != idx {
            offsets.push(i - idx);
        }
    }
    offsets
}

#[test]
fn offsets() {
    let no_offsets: Vec<isize> = Vec::new();
    assert_eq!(make_offsets(0, 1), no_offsets);

    assert_eq!(make_offsets(0, 3), vec![1, 2]);

    assert_eq!(make_offsets(1, 3), vec![-1, 1]);

    assert_eq!(make_offsets(2, 3), vec![-2, -1]);
}

#[test]
fn junction() {
    let d = LaneType::Driving;

    assert_eq!(
        lane_specs_for((vec![d], vec![])),
        vec![LaneSpec::new(d, 0, false, None, vec![])]
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
            LaneSpec::new(d, 0, false, None, vec![1, 2]),
            LaneSpec::new(p, 1, false, None, vec![-1, 1]),
            LaneSpec::new(s, 2, false, Some(1), vec![-2, -1]),
            LaneSpec::new(s, 0, true, Some(-1), vec![]),
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
            LaneSpec::new(d, 0, false, Some(3), vec![1, 2]),
            LaneSpec::new(p, 1, false, None, vec![-1, 1]),
            LaneSpec::new(s, 2, false, Some(3), vec![-2, -1]),
            LaneSpec::new(d, 0, true, Some(-3), vec![1, 2]),
            LaneSpec::new(p, 1, true, None, vec![-1, 1]),
            LaneSpec::new(s, 2, true, Some(-3), vec![-2, -1]),
        ]
    );
}

#[test]
fn big_twoway() {
    let d = LaneType::Driving;
    let b = LaneType::Biking;
    let p = LaneType::Parking;
    let s = LaneType::Sidewalk;

    assert_eq!(
        lane_specs_for((vec![d, d, b, p, s], vec![d, d, b, p, s])),
        vec![
            LaneSpec::new(d, 0, false, Some(5), vec![1, 2, 3, 4]),
            LaneSpec::new(d, 1, false, Some(5), vec![-1, 1, 2, 3]),
            LaneSpec::new(b, 2, false, Some(5), vec![-2, -1, 1, 2]),
            LaneSpec::new(p, 3, false, None, vec![-3, -2, -1, 1]),
            LaneSpec::new(s, 4, false, Some(5), vec![-4, -3, -2, -1]),
            LaneSpec::new(d, 0, true, Some(-5), vec![1, 2, 3, 4]),
            LaneSpec::new(d, 1, true, Some(-5), vec![-1, 1, 2, 3]),
            LaneSpec::new(b, 2, true, Some(-5), vec![-2, -1, 1, 2]),
            LaneSpec::new(p, 3, true, None, vec![-3, -2, -1, 1]),
            LaneSpec::new(s, 4, true, Some(-5), vec![-4, -3, -2, -1]),
        ]
    );
}

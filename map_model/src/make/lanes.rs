use edits::RoadEdits;
use lane::LaneType;
use raw_data;
use road::RoadID;
use std::iter;

// (original direction, reversed direction)
fn get_lanes(r: &raw_data::Road) -> (Vec<LaneType>, Vec<LaneType>) {
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
    let driving_lanes_per_side: Vec<LaneType> = iter::repeat(LaneType::Driving)
        .take(if oneway {
            num_driving_lanes_per_road
        } else {
            // TODO OSM way 124940792 is I5 express lane, should it be considered oneway?
            (num_driving_lanes_per_road / 2).max(1)
        }).collect();

    let has_bike_lane = r.osm_tags.get("cycleway") == Some(&"lane".to_string());
    let has_sidewalk = r.osm_tags.get("highway") != Some(&"motorway".to_string())
        && r.osm_tags.get("highway") != Some(&"motorway_link".to_string());
    let has_parking = has_sidewalk;

    let mut full_side = driving_lanes_per_side;
    if has_bike_lane {
        full_side.push(LaneType::Biking);
    }
    if has_parking {
        full_side.push(LaneType::Parking);
    }
    if has_sidewalk {
        full_side.push(LaneType::Sidewalk);
    }

    if oneway {
        // Only residential highways have a sidewalk on the other side of a highway.
        let other_side =
            if has_sidewalk && r.osm_tags.get("highway") == Some(&"residential".to_string()) {
                vec![LaneType::Sidewalk]
            } else {
                Vec::new()
            };
        (full_side, other_side)
    } else {
        (full_side.clone(), full_side)
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

pub fn get_lane_specs(r: &raw_data::Road, id: RoadID, edits: &RoadEdits) -> Vec<LaneSpec> {
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

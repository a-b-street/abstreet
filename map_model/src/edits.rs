use {Lane, LaneType, Road, RoadID};

#[derive(Serialize, Deserialize, Debug)]
pub enum Reason {
    BasemapWrong,
    Hypothetical,
}

// TODO bring in the intersection modifications from the control crate here. for now, road edits
// are here, since map construction maybe needs to know these?

#[derive(Serialize, Deserialize, Debug)]
pub struct RoadEdit {
    road: RoadID,
    forwards_lanes: Vec<LaneType>,
    backwards_lanes: Vec<LaneType>,
    reason: Reason,
}

impl RoadEdit {
    pub fn change_lane_type(
        reason: Reason,
        r: &Road,
        lane: &Lane,
        new_type: LaneType,
    ) -> Option<RoadEdit> {
        let (mut forwards, mut backwards) = r.get_lane_types();
        let (is_fwd, idx) = r.dir_and_offset(lane.id);
        if is_fwd {
            assert_ne!(forwards[idx], new_type);
            forwards[idx] = new_type;
            if !are_lanes_valid(&forwards) {
                return None;
            }
        } else {
            assert_ne!(backwards[idx], new_type);
            backwards[idx] = new_type;
            if !are_lanes_valid(&backwards) {
                return None;
            }
        }

        Some(RoadEdit {
            road: r.id,
            forwards_lanes: forwards,
            backwards_lanes: backwards,
            reason,
        })
    }

    pub fn delete_lane(r: &Road, lane: &Lane) -> Option<RoadEdit> {
        // Sidewalks are fixed
        if lane.lane_type == LaneType::Sidewalk {
            return None;
        }

        let (mut forwards, mut backwards) = r.get_lane_types();
        let (is_fwd, idx) = r.dir_and_offset(lane.id);
        if is_fwd {
            forwards.remove(idx);
        } else {
            backwards.remove(idx);
        }

        Some(RoadEdit {
            road: r.id,
            forwards_lanes: forwards,
            backwards_lanes: backwards,
            reason: Reason::BasemapWrong,
        })
    }
}

fn are_lanes_valid(lanes: &Vec<LaneType>) -> bool {
    // Can't have adjacent parking lanes
    for pair in lanes.windows(2) {
        if pair[0] == LaneType::Parking && pair[1] == LaneType::Parking {
            return false;
        }
    }

    // Can't have two sidewalks on one side of a road
    if lanes.iter().filter(|&&lt| lt == LaneType::Sidewalk).count() > 1 {
        return false;
    }

    // I'm sure other ideas will come up. :)

    true
}

use std::collections::BTreeMap;
use {Lane, LaneType, Road, RoadID};

// TODO bring in the intersection modifications from the control crate here. for now, road edits
// are here, since map construction maybe needs to know these?
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Edits {
    // TODO detect when we wind up editing back to the original thing
    pub(crate) roads: BTreeMap<RoadID, RoadEdit>,
}

impl Edits {
    pub fn new() -> Edits {
        Edits {
            roads: BTreeMap::new(),
        }
    }

    pub fn change_lane_type(
        &mut self,
        reason: EditReason,
        r: &Road,
        lane: &Lane,
        new_type: LaneType,
    ) -> bool {
        if let Some(edit) = RoadEdit::change_lane_type(reason, r, lane, new_type) {
            self.roads.insert(r.id, edit);
            return true;
        }
        false
    }

    pub fn delete_lane(&mut self, r: &Road, lane: &Lane) -> bool {
        if let Some(edit) = RoadEdit::delete_lane(r, lane) {
            self.roads.insert(r.id, edit);
            return true;
        }
        false
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum EditReason {
    BasemapWrong,
    Hypothetical,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoadEdit {
    road: RoadID,
    pub(crate) forwards_lanes: Vec<LaneType>,
    pub(crate) backwards_lanes: Vec<LaneType>,
    reason: EditReason,
}

impl RoadEdit {
    // TODO return Result, so we can enforce a reason coming back!
    fn change_lane_type(
        reason: EditReason,
        r: &Road,
        lane: &Lane,
        new_type: LaneType,
    ) -> Option<RoadEdit> {
        if lane.is_sidewalk() {
            error!("Sidewalks are fixed; can't change their type");
            return None;
        }

        let (mut forwards, mut backwards) = r.get_lane_types();
        let (is_fwd, idx) = r.dir_and_offset(lane.id);
        if is_fwd {
            if forwards[idx] == new_type {
                error!("{} is already {:?}", lane.id, new_type);
                return None;
            }
            forwards[idx] = new_type;
            if !are_lanes_valid(&forwards) {
                return None;
            }
        } else {
            if backwards[idx] == new_type {
                error!("{} is already {:?}", lane.id, new_type);
                return None;
            }
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

    fn delete_lane(r: &Road, lane: &Lane) -> Option<RoadEdit> {
        // Sidewalks are fixed
        if lane.is_sidewalk() {
            error!("Can't delete sidewalks");
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
            reason: EditReason::BasemapWrong,
        })
    }
}

fn are_lanes_valid(lanes: &Vec<LaneType>) -> bool {
    // TODO this check doesn't seem to be working
    for pair in lanes.windows(2) {
        if pair[0] == LaneType::Parking && pair[1] == LaneType::Parking {
            error!("Can't have two adjacent parking lanes");
            return false;
        }
    }

    // Can't have two sidewalks on one side of a road
    if lanes.iter().filter(|&&lt| lt == LaneType::Sidewalk).count() > 1 {
        error!("Can't have two sidewalks on one side of a road");
        return false;
    }

    // I'm sure other ideas will come up. :)

    true
}

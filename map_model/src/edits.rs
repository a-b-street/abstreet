use crate::{ControlStopSign, ControlTrafficSignal, IntersectionID, Lane, LaneType, Road, RoadID};
use abstutil;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapEdits {
    pub edits_name: String,
    pub map_name: String,

    // TODO detect when we wind up editing back to the original thing
    pub(crate) roads: BTreeMap<RoadID, RoadEdit>,
    pub(crate) stop_signs: BTreeMap<IntersectionID, ControlStopSign>,
    pub(crate) traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal>,
}

impl MapEdits {
    pub fn new(map_name: &str) -> MapEdits {
        MapEdits {
            // Something has to fill this out later
            edits_name: "no_edits".to_string(),
            map_name: map_name.to_string(),
            roads: BTreeMap::new(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "map edits \"{}\" ({} roads, {} stop signs, {} traffic signals",
            self.edits_name,
            self.roads.len(),
            self.stop_signs.len(),
            self.traffic_signals.len()
        )
    }

    pub fn save(&self) {
        abstutil::save_object("edits", &self.map_name, &self.edits_name, self);
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

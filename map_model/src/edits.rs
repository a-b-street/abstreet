use crate::raw_data::StableRoadID;
use crate::{ControlStopSign, ControlTrafficSignal, IntersectionID, Lane, LaneType, Road};
use abstutil::Error;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapEdits {
    pub edits_name: String,
    pub map_name: String,

    // TODO detect when we wind up editing back to the original thing
    pub(crate) roads: BTreeMap<StableRoadID, RoadEdit>,
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

    pub fn can_change_lane_type(&self, r: &Road, lane: &Lane, new_type: LaneType) -> bool {
        RoadEdit::change_lane_type(EditReason::BasemapWrong, r, lane, new_type).is_some()
    }

    pub fn change_lane_type(
        &mut self,
        reason: EditReason,
        r: &Road,
        lane: &Lane,
        new_type: LaneType,
    ) {
        let edit = RoadEdit::change_lane_type(reason, r, lane, new_type).unwrap();
        self.roads.insert(r.stable_id, edit);
    }

    pub fn delete_lane(&mut self, r: &Road, lane: &Lane) {
        self.roads
            .insert(r.stable_id, RoadEdit::delete_lane(r, lane));
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum EditReason {
    BasemapWrong,
    Hypothetical,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoadEdit {
    road: StableRoadID,
    pub(crate) forwards_lanes: Vec<LaneType>,
    pub(crate) backwards_lanes: Vec<LaneType>,
    reason: EditReason,
}

impl RoadEdit {
    fn change_lane_type(
        reason: EditReason,
        r: &Road,
        lane: &Lane,
        new_type: LaneType,
    ) -> Option<RoadEdit> {
        if lane.is_sidewalk() {
            panic!("Sidewalks are fixed; can't change their type");
        }

        let (mut forwards, mut backwards) = r.get_lane_types();
        let (is_fwd, idx) = r.dir_and_offset(lane.id);
        if is_fwd {
            if forwards[idx] == new_type {
                panic!("{} is already {:?}", lane.id, new_type);
            }
            forwards[idx] = new_type;
            if let Err(err) = are_lanes_valid(&forwards) {
                println!("{}", err);
                return None;
            }
        } else {
            if backwards[idx] == new_type {
                panic!("{} is already {:?}", lane.id, new_type);
            }
            backwards[idx] = new_type;
            if let Err(err) = are_lanes_valid(&backwards) {
                println!("{}", err);
                return None;
            }
        }

        Some(RoadEdit {
            road: r.stable_id,
            forwards_lanes: forwards,
            backwards_lanes: backwards,
            reason,
        })
    }

    fn delete_lane(r: &Road, lane: &Lane) -> RoadEdit {
        if lane.is_sidewalk() {
            panic!("Can't delete sidewalks");
        }

        let (mut forwards, mut backwards) = r.get_lane_types();
        let (is_fwd, idx) = r.dir_and_offset(lane.id);
        if is_fwd {
            forwards.remove(idx);
        } else {
            backwards.remove(idx);
        }

        RoadEdit {
            road: r.stable_id,
            forwards_lanes: forwards,
            backwards_lanes: backwards,
            reason: EditReason::BasemapWrong,
        }
    }
}

fn are_lanes_valid(lanes: &Vec<LaneType>) -> Result<(), Error> {
    // TODO this check doesn't seem to be working
    for pair in lanes.windows(2) {
        if pair[0] == LaneType::Parking && pair[1] == LaneType::Parking {
            return Err(Error::new(
                "Can't have two adjacent parking lanes".to_string(),
            ));
        }
    }

    // Can't have two sidewalks on one side of a road
    if lanes.iter().filter(|&&lt| lt == LaneType::Sidewalk).count() > 1 {
        return Err(Error::new(
            "Can't have two sidewalks on one side of a road".to_string(),
        ));
    }

    // I'm sure other ideas will come up. :)

    Ok(())
}

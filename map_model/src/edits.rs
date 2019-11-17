use crate::{
    ControlStopSign, ControlTrafficSignal, IntersectionID, IntersectionType, LaneID, LaneType,
    RoadID, TurnID,
};
use abstutil::Timer;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapEdits {
    pub(crate) map_name: String,
    pub edits_name: String,
    pub commands: Vec<EditCmd>,

    #[serde(skip_serializing, skip_deserializing)]
    pub dirty: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum EditCmd {
    ChangeLaneType {
        id: LaneID,
        lt: LaneType,
        orig_lt: LaneType,
    },
    ReverseLane {
        l: LaneID,
        dst_i: IntersectionID,
    },
    ChangeStopSign(ControlStopSign),
    ChangeTrafficSignal(ControlTrafficSignal),
    CloseIntersection {
        id: IntersectionID,
        orig_it: IntersectionType,
    },
    UncloseIntersection(IntersectionID, IntersectionType),
}

pub struct EditEffects {
    pub changed_lanes: BTreeSet<LaneID>,
    pub changed_roads: BTreeSet<RoadID>,
    pub changed_intersections: BTreeSet<IntersectionID>,
    pub added_turns: BTreeSet<TurnID>,
    pub deleted_turns: BTreeSet<TurnID>,
}

impl MapEdits {
    pub fn new(map_name: String) -> MapEdits {
        MapEdits {
            map_name,
            // Something has to fill this out later
            edits_name: "no_edits".to_string(),
            commands: Vec::new(),
            dirty: false,
        }
    }

    pub fn load(map_name: &str, edits_name: &str, timer: &mut Timer) -> MapEdits {
        if edits_name == "no_edits" {
            return MapEdits::new(map_name.to_string());
        }
        abstutil::read_json(
            &abstutil::path1_json(map_name, abstutil::EDITS, edits_name),
            timer,
        )
        .unwrap()
    }

    // TODO Version these
    pub(crate) fn save(&mut self) {
        assert!(self.dirty);
        assert_ne!(self.edits_name, "no_edits");
        abstutil::save_json_object(abstutil::EDITS, &self.map_name, &self.edits_name, self);
        self.dirty = false;
    }

    pub fn original_it(&self, i: IntersectionID) -> IntersectionType {
        for cmd in &self.commands {
            if let EditCmd::CloseIntersection { id, orig_it } = cmd {
                if *id == i {
                    return *orig_it;
                }
            }
        }
        panic!("{} isn't closed", i);
    }
}

impl EditEffects {
    pub fn new() -> EditEffects {
        EditEffects {
            changed_lanes: BTreeSet::new(),
            changed_roads: BTreeSet::new(),
            changed_intersections: BTreeSet::new(),
            added_turns: BTreeSet::new(),
            deleted_turns: BTreeSet::new(),
        }
    }
}

use abstutil;
use control::{ControlStopSign, ControlTrafficSignal};
use map_model::{IntersectionID, RoadEdits};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Clone)]
pub struct MapEdits {
    pub edits_name: String,
    pub map_name: String,

    pub road_edits: RoadEdits,
    pub stop_signs: BTreeMap<IntersectionID, ControlStopSign>,
    pub traffic_signals: BTreeMap<IntersectionID, ControlTrafficSignal>,
}

impl MapEdits {
    pub fn new() -> MapEdits {
        MapEdits {
            edits_name: "no_edits".to_string(),
            map_name: "TODO".to_string(), // TODO er
            road_edits: RoadEdits::new(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
        }
    }

    pub fn describe(&self) -> String {
        format!(
            "map edits \"{}\" ({} roads, {} stop signs, {} traffic signals",
            self.edits_name,
            self.road_edits.len(),
            self.stop_signs.len(),
            self.traffic_signals.len()
        )
    }

    pub fn save(&self) {
        abstutil::save_object("edits", &self.map_name, &self.edits_name, self);
    }
}

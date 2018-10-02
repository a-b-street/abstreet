use control::{ModifiedStopSign, ModifiedTrafficSignal};
use map_model::{IntersectionID, RoadEdits};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
pub struct MapEdits {
    pub edits_name: String,
    pub map_name: String,

    pub road_edits: RoadEdits,
    pub stop_signs: BTreeMap<IntersectionID, ModifiedStopSign>,
    pub traffic_signals: BTreeMap<IntersectionID, ModifiedTrafficSignal>,
}

impl MapEdits {
    pub fn new() -> MapEdits {
        MapEdits {
            edits_name: "unnamed".to_string(),
            map_name: "TODO".to_string(), // TODO er
            road_edits: RoadEdits::new(),
            stop_signs: BTreeMap::new(),
            traffic_signals: BTreeMap::new(),
        }
    }
}

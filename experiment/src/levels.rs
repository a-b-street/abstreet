use serde::{Deserialize, Serialize};

use abstutil::MapName;
use geom::Duration;
use map_model::osm;

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Level {
    pub title: String,
    pub map: MapName,
    pub start: osm::NodeID,
    pub minimap_zoom: usize,
    pub time_limit: Duration,
    pub goal: usize,

    pub unlock_upzones: usize,
    pub unlock_vehicles: Vec<String>,
}

impl Level {
    pub fn all() -> Vec<Level> {
        vec![
            Level {
                title: "University District".to_string(),
                map: MapName::seattle("udistrict_ravenna"),
                start: osm::NodeID(53162661),
                minimap_zoom: 1,
                time_limit: Duration::seconds(90.0),
                goal: 25,

                unlock_upzones: 2,
                unlock_vehicles: vec!["bike".to_string()],
            },
            Level {
                title: "Wallingford".to_string(),
                map: MapName::seattle("wallingford"),
                start: osm::NodeID(53218389),
                minimap_zoom: 2,
                time_limit: Duration::seconds(90.0),
                goal: 25,

                unlock_upzones: 2,
                unlock_vehicles: vec!["cargo bike".to_string()],
            },
            // TODO Super dense, starting point isn't even near apartments, run out of gifts after
            // a few buildings. Unexpectedly hard!
            Level {
                title: "South Lake Union".to_string(),
                map: MapName::seattle("slu"),
                start: osm::NodeID(53142423),
                minimap_zoom: 1,
                time_limit: Duration::seconds(90.0),
                goal: 25,

                unlock_upzones: 2,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Montlake".to_string(),
                map: MapName::seattle("montlake"),
                start: osm::NodeID(53084814),
                minimap_zoom: 1,
                time_limit: Duration::seconds(30.0),
                goal: 25,

                unlock_upzones: 2,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Magnolia".to_string(),
                map: MapName::seattle("ballard"),
                start: osm::NodeID(53117102),
                minimap_zoom: 2,
                time_limit: Duration::minutes(5),
                goal: 1000,

                unlock_upzones: 3,
                unlock_vehicles: vec![],
            },
        ]
    }
}

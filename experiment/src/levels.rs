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
    // TODO Like Challenge::all; cache with lazy static?
    pub fn all() -> Vec<Level> {
        vec![
            Level {
                title: "Level 1 - a small neighborhood".to_string(),
                map: MapName::seattle("montlake"),
                start: osm::NodeID(53084814),
                minimap_zoom: 1,
                time_limit: Duration::seconds(30.0),
                goal: 25,

                unlock_upzones: 2,
                unlock_vehicles: vec!["bike".to_string(), "cargo bike".to_string()],
            },
            Level {
                title: "Level 2 - a small neighborhood with upzones".to_string(),
                map: MapName::seattle("montlake"),
                start: osm::NodeID(53084814),
                minimap_zoom: 1,
                time_limit: Duration::minutes(4),
                goal: 1000,

                unlock_upzones: 3,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Level 3 - Magnolia".to_string(),
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

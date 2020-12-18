use serde::{Deserialize, Serialize};

use abstutil::MapName;
use geom::Duration;
use map_model::osm;

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Level {
    pub title: String,
    pub description: String,
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
                description: "Tear yourself away from all the bubble tea to deliver presents to \
                              some college students, whether they've been naughty or nice."
                    .to_string(),
                map: MapName::seattle("udistrict_ravenna"),
                start: osm::NodeID(53162661),
                minimap_zoom: 1,
                time_limit: Duration::seconds(90.0),
                goal: 1000,

                unlock_upzones: 1,
                unlock_vehicles: vec!["sleigh".to_string()],
            },
            Level {
                title: "Wallingfjord".to_string(),
                description: "Stone and 45th have food aplenty, but can you manage deliveries to \
                              everyone tucked away in the neighborhood?"
                    .to_string(),
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
                title: "South Pole Union".to_string(),
                description: "Don't get turned around in all of the construction while you \
                              deliver to the apartments here!"
                    .to_string(),
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
                description: "With the Montlake Market closed, how will you manage to bring cheer \
                              to this sleepy little pocket of the city?"
                    .to_string(),
                map: MapName::seattle("montlake"),
                start: osm::NodeID(53084814),
                minimap_zoom: 1,
                time_limit: Duration::minutes(3),
                goal: 1000,

                unlock_upzones: 2,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Magnolia".to_string(),
                description: "Struggle past the intense hills and restrictive zoning to tackle \
                              one of the lowest-density parts of Seattle!"
                    .to_string(),
                map: MapName::seattle("ballard"),
                start: osm::NodeID(53117102),
                minimap_zoom: 2,
                time_limit: Duration::minutes(5),
                goal: 1000,

                unlock_upzones: 3,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Phinney Ridge".to_string(),
                description: "...".to_string(),
                map: MapName::seattle("phinney"),
                start: osm::NodeID(53233319),
                minimap_zoom: 1,
                time_limit: Duration::minutes(5),
                goal: 1000,

                unlock_upzones: 3,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Queen Anne".to_string(),
                description: "...".to_string(),
                map: MapName::seattle("qa"),
                start: osm::NodeID(53234637),
                minimap_zoom: 1,
                time_limit: Duration::minutes(5),
                goal: 1000,

                unlock_upzones: 3,
                unlock_vehicles: vec![],
            },
        ]
    }
}

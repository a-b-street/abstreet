use serde::{Deserialize, Serialize};

use abstio::MapName;
use geom::Duration;
use map_model::osm;

#[derive(Serialize, Deserialize, PartialEq, Clone)]
pub struct Level {
    pub title: String,
    pub description: String,
    pub map: MapName,
    pub music: String,
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
                title: "Queen Anne".to_string(),
                description: "Nice hilltop views, beautiful houses -- but are they far from \
                              stores?"
                    .to_string(),
                map: MapName::seattle("qa"),
                music: "jingle_bells".to_string(),
                start: osm::NodeID(53234637),
                minimap_zoom: 1,
                time_limit: Duration::seconds(90.0),
                goal: 350,

                unlock_upzones: 1,
                unlock_vehicles: vec![],
            },
            Level {
                title: "University District".to_string(),
                description: "Tear yourself away from all the bubble tea to deliver presents to \
                              some college students, whether they've been naughty or nice."
                    .to_string(),
                map: MapName::seattle("udistrict_ravenna"),
                music: "god_rest_ye_merry_gentlemen".to_string(),
                start: osm::NodeID(53162661),
                minimap_zoom: 1,
                time_limit: Duration::minutes(2),
                goal: 1500,

                unlock_upzones: 1,
                unlock_vehicles: vec!["cargo bike".to_string()],
            },
            Level {
                title: "Wallingfjord".to_string(),
                description: "Stone and 45th have food aplenty, but can you manage deliveries to \
                              everyone tucked away in the neighborhood?"
                    .to_string(),
                map: MapName::seattle("wallingford"),
                music: "silent_night".to_string(),
                start: osm::NodeID(53218389),
                minimap_zoom: 2,
                time_limit: Duration::minutes(3),
                goal: 1500,

                unlock_upzones: 1,
                unlock_vehicles: vec!["sleigh".to_string()],
            },
            Level {
                title: "Montlake".to_string(),
                description: "With the Montlake Market closed, how will you manage to bring cheer \
                              to this sleepy little pocket of the city?"
                    .to_string(),
                map: MapName::seattle("montlake"),
                music: "dance_sugar_plum_fairy".to_string(),
                start: osm::NodeID(53084814),
                minimap_zoom: 1,
                time_limit: Duration::minutes(3),
                goal: 1000,

                unlock_upzones: 1,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Phinney Ridge".to_string(),
                description: "Take your pick from the scrumptious options along Greenwood Ave! \
                              But stray into the neighborhood at your own risk..."
                    .to_string(),
                map: MapName::seattle("phinney"),
                music: "silent_night".to_string(),
                start: osm::NodeID(53233319),
                minimap_zoom: 1,
                time_limit: Duration::minutes(3),
                goal: 1500,

                unlock_upzones: 1,
                unlock_vehicles: vec![],
            },
            Level {
                title: "South Pole Union".to_string(),
                description: "Suddenly, shops everywhere! Can you find all of the residents \
                              huddled inside?"
                    .to_string(),
                map: MapName::seattle("slu"),
                music: "carol_bells".to_string(),
                start: osm::NodeID(53142423),
                minimap_zoom: 1,
                time_limit: Duration::seconds(90.0),
                goal: 1300,

                unlock_upzones: 3,
                unlock_vehicles: vec![],
            },
            Level {
                title: "Magnolia".to_string(),
                description: "Struggle past the intense hills and restrictive zoning to tackle \
                              one of the lowest-density parts of Seattle!"
                    .to_string(),
                map: MapName::seattle("ballard"),
                music: "god_rest_ye_merry_gentlemen".to_string(),
                start: osm::NodeID(53130883),
                minimap_zoom: 2,
                time_limit: Duration::minutes(4),
                goal: 5000,

                unlock_upzones: 5,
                unlock_vehicles: vec![],
            },
        ]
    }
}

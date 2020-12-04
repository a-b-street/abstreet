use abstutil::MapName;
use geom::Duration;
use map_model::osm;

#[derive(Clone)]
pub struct Level {
    pub title: &'static str,
    pub map: MapName,
    pub start: osm::NodeID,
    pub minimap_zoom: usize,
    pub num_upzones: usize,
    pub vehicles: Vec<&'static str>,
    pub time_limit: Duration,
    pub goal: usize,
}

impl Level {
    // TODO Like Challenge::all; cache with lazy static?
    pub fn all() -> Vec<Level> {
        vec![
            Level {
                title: "Level 1 - a small neighborhood",
                map: MapName::seattle("montlake"),
                start: osm::NodeID(53084814),
                minimap_zoom: 1,
                num_upzones: 0,
                vehicles: vec!["sleigh"],
                time_limit: Duration::minutes(1),
                goal: 1000,
            },
            Level {
                title: "Level 2 - a small neighborhood with upzones",
                map: MapName::seattle("montlake"),
                start: osm::NodeID(53084814),
                minimap_zoom: 1,
                num_upzones: 3,
                vehicles: vec!["bike", "cargo bike", "sleigh"],
                time_limit: Duration::minutes(4),
                goal: 1000,
            },
            Level {
                title: "Level 3 - Magnolia",
                map: MapName::seattle("ballard"),
                start: osm::NodeID(53117102),
                minimap_zoom: 2,
                num_upzones: 5,
                vehicles: vec!["bike", "cargo bike", "sleigh"],
                time_limit: Duration::minutes(5),
                goal: 1000,
            },
        ]
    }
}

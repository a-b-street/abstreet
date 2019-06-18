use crate::{LaneID, Position};
use abstutil;
use geom::{Line, Polygon};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BuildingID(pub usize);

impl fmt::Display for BuildingID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BuildingID({0})", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FrontPath {
    pub bldg: BuildingID,
    pub sidewalk: Position,
    // Goes from the building to the sidewalk
    pub line: Line,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Building {
    pub id: BuildingID,
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,

    pub front_path: FrontPath,
}

impl Building {
    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }

    pub fn sidewalk(&self) -> LaneID {
        self.front_path.sidewalk.lane()
    }

    pub fn get_name(&self) -> String {
        let address = match (
            self.osm_tags.get("addr:housenumber"),
            self.osm_tags.get("addr:street"),
        ) {
            (Some(num), Some(st)) => format!("{} {}", num, st),
            (None, Some(st)) => format!("??? {}", st),
            _ => "???".to_string(),
        };
        if let Some(name) = self.osm_tags.get("name") {
            format!("{} (at {})", name, address)
        } else {
            address
        }
    }
}

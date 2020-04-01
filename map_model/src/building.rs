use crate::{osm, LaneID, Map, Position};
use geom::{Line, Polygon, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BuildingID(pub usize);

impl fmt::Display for BuildingID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Building #{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FrontPath {
    pub sidewalk: Position,
    // Goes from the building to the sidewalk
    pub line: Line,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct OffstreetParking {
    pub name: String,
    pub num_stalls: usize,
    // Goes from the building to the driving lane
    pub driveway_line: Line,
    // Guaranteed to be at least 7m before the end of the lane
    pub driving_pos: Position,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Building {
    pub id: BuildingID,
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
    // Where a text label should be centered to have the best chances of being contained within the
    // polygon.
    pub label_center: Pt2D,
    // (Name, amenity)
    pub amenities: BTreeSet<(String, String)>,

    pub front_path: FrontPath,
    pub parking: Option<OffstreetParking>,
}

impl Building {
    pub fn sidewalk(&self) -> LaneID {
        self.front_path.sidewalk.lane()
    }

    pub fn just_address(&self, map: &Map) -> String {
        match (
            self.osm_tags.get("addr:housenumber"),
            self.osm_tags.get("addr:street"),
        ) {
            (Some(num), Some(st)) => format!("{} {}", num, st),
            (None, Some(st)) => format!("??? {}", st),
            _ => format!("??? {}", map.get_parent(self.sidewalk()).get_name()),
        }
    }

    pub fn just_name(&self) -> Option<&String> {
        self.osm_tags.get(osm::NAME)
    }

    // TODO I think this one only has one caller
    pub fn get_name(&self, map: &Map) -> String {
        let address = self.just_address(map);
        if let Some(name) = self.just_name() {
            format!("{} (at {})", name, address)
        } else {
            address
        }
    }
}

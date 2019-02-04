use crate::{LaneID, Position};
use abstutil;
use geom::{Line, Pt2D};
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
    pub points: Vec<Pt2D>,
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
}

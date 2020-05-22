use geom::Polygon;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AreaID(pub usize);

impl fmt::Display for AreaID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Area #{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum AreaType {
    Park,
    Water,
    PedestrianIsland,
    Island,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Area {
    pub id: AreaID,
    pub area_type: AreaType,
    pub polygon: Polygon,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_id: i64,
}

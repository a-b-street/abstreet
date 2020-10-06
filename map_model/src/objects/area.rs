// Areas are just used for drawing.

use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize, Tags};
use geom::Polygon;

use crate::osm;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AreaID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

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
    pub osm_tags: Tags,
    pub osm_id: osm::OsmID,
}

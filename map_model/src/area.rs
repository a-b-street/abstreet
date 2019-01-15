use crate::LANE_THICKNESS;
use abstutil;
use geom::{PolyLine, Polygon, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AreaID(pub usize);

impl fmt::Display for AreaID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AreaID({0})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum AreaType {
    Park,
    Swamp,
    Water,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Area {
    pub id: AreaID,
    pub area_type: AreaType,
    // Might be a closed loop or not -- waterways can be linear.
    pub points: Vec<Pt2D>,
    pub osm_tags: BTreeMap<String, String>,
    pub osm_way_id: i64,
}

impl Area {
    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
        println!("{}", PolyLine::new(self.points.clone()));
    }

    pub fn get_polygon(&self) -> Polygon {
        if self.points[0] == *self.points.last().unwrap() {
            return Polygon::new(&self.points);
        }
        PolyLine::new(self.points.clone()).make_polygons(LANE_THICKNESS)
    }
}

use abstutil;
use std::fmt;
use {LaneID, Position};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BusStopID {
    pub sidewalk: LaneID,
    pub idx: usize,
}

impl fmt::Display for BusStopID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BusStopID({0}, {1})", self.sidewalk, self.idx)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BusRouteID(pub usize);

impl fmt::Display for BusRouteID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BusRouteID({0})", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BusStop {
    pub id: BusStopID,
    pub driving_pos: Position,
    pub sidewalk_pos: Position,
}

impl BusStop {
    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BusRoute {
    pub id: BusRouteID,
    pub name: String,
    pub stops: Vec<BusStopID>,
}

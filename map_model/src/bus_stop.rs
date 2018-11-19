use abstutil;
use dimensioned::si;
use std::fmt;
use LaneID;

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BusStop {
    pub id: BusStopID,
    pub driving_lane: LaneID,
    pub dist_along: si::Meter<f64>,
}

impl BusStop {
    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }
}

// TODO This sort of doesn't fit in the map layer, but it's quite convenient to store it.
#[derive(Serialize, Deserialize, Debug)]
pub struct BusRoute {
    pub name: String,
    pub stops: Vec<BusStopID>,
}

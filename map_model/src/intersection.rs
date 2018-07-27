// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use geom::Pt2D;
use std::fmt;
use {LaneID, TurnID, RoadID, Map};
use std::collections::HashSet;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IntersectionID(pub usize);

impl fmt::Display for IntersectionID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "IntersectionID({0})", self.0)
    }
}

#[derive(Debug)]
pub struct Intersection {
    pub id: IntersectionID,
    pub point: Pt2D,
    pub turns: Vec<TurnID>,
    pub elevation: si::Meter<f64>,
    pub has_traffic_signal: bool,

    // Some duplication that's proving convenient so far
    pub incoming_lanes: Vec<LaneID>,
    pub outgoing_lanes: Vec<LaneID>,
}

impl PartialEq for Intersection {
    fn eq(&self, other: &Intersection) -> bool {
        self.id == other.id
    }
}

impl Intersection {
    pub fn is_dead_end(&self, map: &Map) -> bool {
        let mut roads: HashSet<RoadID> = HashSet::new();
        for l in self.incoming_lanes.iter().chain(self.outgoing_lanes.iter()) {
            roads.insert(map.get_l(*l).parent);
        }
        roads.len() == 1
    }
}

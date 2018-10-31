// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use dimensioned::si;
use geom::Pt2D;
use std::collections::BTreeSet;
use std::fmt;
use {LaneID, Map, RoadID, TurnID};

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IntersectionID(pub usize);

impl fmt::Display for IntersectionID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "IntersectionID({0})", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Intersection {
    pub id: IntersectionID,
    pub point: Pt2D,
    // TODO This should really be a Polygon, but it's hard to construct in the right order and
    // weird to represent an Option<Polygon> during construction.
    pub polygon: Vec<Pt2D>,
    pub turns: Vec<TurnID>,
    pub elevation: si::Meter<f64>,
    pub has_traffic_signal: bool,

    // Note that a lane may belong to both incoming_lanes and outgoing_lanes.
    // TODO narrow down when and why. is it just sidewalks in weird cases?
    pub incoming_lanes: Vec<LaneID>,
    pub outgoing_lanes: Vec<LaneID>,
}

impl PartialEq for Intersection {
    fn eq(&self, other: &Intersection) -> bool {
        self.id == other.id
    }
}

impl Intersection {
    pub fn get_roads(&self, map: &Map) -> BTreeSet<RoadID> {
        let mut roads: BTreeSet<RoadID> = BTreeSet::new();
        for l in self.incoming_lanes.iter().chain(self.outgoing_lanes.iter()) {
            roads.insert(map.get_l(*l).parent);
        }
        roads
    }

    pub fn is_dead_end(&self, map: &Map) -> bool {
        self.get_roads(map).len() == 1
    }

    pub fn is_degenerate(&self, map: &Map) -> bool {
        self.get_roads(map).len() == 2
    }

    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }
}

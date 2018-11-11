// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use abstutil;
use dimensioned::si;
use geom::Pt2D;
use std::collections::BTreeSet;
use std::fmt;
use {LaneID, RoadID, TurnID};

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IntersectionID(pub usize);

impl fmt::Display for IntersectionID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "IntersectionID({0})", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum IntersectionType {
    StopSign,
    TrafficSignal,
    Border,
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

    pub intersection_type: IntersectionType,

    // Note that a lane may belong to both incoming_lanes and outgoing_lanes.
    // TODO narrow down when and why. is it just sidewalks in weird cases?
    pub incoming_lanes: Vec<LaneID>,
    pub outgoing_lanes: Vec<LaneID>,

    pub roads: BTreeSet<RoadID>,
}

impl PartialEq for Intersection {
    fn eq(&self, other: &Intersection) -> bool {
        self.id == other.id
    }
}

impl Intersection {
    pub fn is_dead_end(&self) -> bool {
        self.roads.len() == 1
    }

    pub fn is_degenerate(&self) -> bool {
        self.roads.len() == 2
    }

    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }
}

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use crate::{LaneID, LaneType, Map, RoadID, TurnID};
use abstutil;
use dimensioned::si;
use geom::Pt2D;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

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

impl Intersection {
    pub fn is_dead_end(&self) -> bool {
        self.roads.len() == 1
    }

    pub fn is_degenerate(&self) -> bool {
        self.roads.len() == 2
    }

    pub fn get_incoming_lanes(&self, map: &Map, lt: LaneType) -> Vec<LaneID> {
        self.incoming_lanes
            .iter()
            .filter(|l| map.get_l(**l).lane_type == lt)
            .cloned()
            .collect()
    }

    pub fn get_outgoing_lanes(&self, map: &Map, lt: LaneType) -> Vec<LaneID> {
        self.outgoing_lanes
            .iter()
            .filter(|l| map.get_l(**l).lane_type == lt)
            .cloned()
            .collect()
    }

    pub fn get_roads_sorted_by_incoming_angle(&self, map: &Map) -> Vec<RoadID> {
        let mut roads: Vec<RoadID> = self.roads.iter().cloned().collect();
        roads.sort_by_key(|id| {
            let r = map.get_r(*id);
            let last_line = if r.dst_i == self.id {
                r.center_pts.last_line()
            } else {
                r.center_pts.first_line().reverse()
            };
            last_line.angle().normalized_degrees() as i64
        });
        roads
    }

    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }
}

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use geom::{Distance, Polygon};

use crate::{osm, DirectedRoadID, LaneID, Map, PathConstraints, Road, RoadID, TurnID};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IntersectionID(
    #[serde(
        serialize_with = "serialize_usize",
        deserialize_with = "deserialize_usize"
    )]
    pub usize,
);

impl fmt::Display for IntersectionID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Intersection #{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum IntersectionType {
    StopSign,
    TrafficSignal,
    Border,
    Construction,
}

/// An intersection connects roads. Most have >2 roads and are controlled by stop signs or traffic
/// signals. Roads that lead to the boundary of the map end at border intersections, with only that
/// one road attached.
#[derive(Serialize, Deserialize, Debug)]
pub struct Intersection {
    pub id: IntersectionID,
    /// This needs to be in clockwise orientation, or later rendering of sidewalk corners breaks.
    pub polygon: Polygon,
    pub turns: BTreeSet<TurnID>,
    pub elevation: Distance,

    pub intersection_type: IntersectionType,
    pub orig_id: osm::NodeID,

    /// Note that a lane may belong to both incoming_lanes and outgoing_lanes.
    // TODO narrow down when and why. is it just sidewalks in weird cases?
    // TODO Change to BTreeSet, or otherwise emphasize to callers that the order of these isn't
    // meaningful
    pub incoming_lanes: Vec<LaneID>,
    pub outgoing_lanes: Vec<LaneID>,

    // TODO Maybe DirectedRoadIDs
    pub roads: BTreeSet<RoadID>,

    /// Was a short road adjacent to this intersection merged?
    pub merged: bool,
}

impl Intersection {
    pub fn is_border(&self) -> bool {
        self.intersection_type == IntersectionType::Border
    }
    pub fn is_incoming_border(&self) -> bool {
        self.intersection_type == IntersectionType::Border && !self.outgoing_lanes.is_empty()
    }
    pub fn is_outgoing_border(&self) -> bool {
        self.intersection_type == IntersectionType::Border && !self.incoming_lanes.is_empty()
    }

    pub fn is_closed(&self) -> bool {
        self.intersection_type == IntersectionType::Construction
    }

    pub fn is_stop_sign(&self) -> bool {
        self.intersection_type == IntersectionType::StopSign
    }

    pub fn is_traffic_signal(&self) -> bool {
        self.intersection_type == IntersectionType::TrafficSignal
    }

    pub fn is_light_rail(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| map.get_r(*r).is_light_rail())
    }

    pub fn is_private(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| map.get_r(*r).is_private())
    }

    pub fn is_footway(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| map.get_r(*r).is_footway())
    }

    pub fn get_incoming_lanes(&self, map: &Map, constraints: PathConstraints) -> Vec<LaneID> {
        self.incoming_lanes
            .iter()
            .filter(move |l| constraints.can_use(map.get_l(**l), map))
            .cloned()
            .collect()
    }

    /// Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub fn get_outgoing_lanes(&self, map: &Map, constraints: PathConstraints) -> Vec<LaneID> {
        constraints.filter_lanes(self.outgoing_lanes.clone(), map)
    }

    pub fn get_zorder(&self, map: &Map) -> isize {
        // TODO Not sure min makes sense -- what about a 1 and a 0? Prefer the nonzeros. If there's
        // a -1 and a 1... need to see it to know what to do.
        self.roads
            .iter()
            .map(|r| map.get_r(*r).zorder)
            .min()
            .unwrap()
    }

    pub fn get_rank(&self, map: &Map) -> osm::RoadRank {
        self.roads
            .iter()
            .map(|r| map.get_r(*r).get_rank())
            .max()
            .unwrap()
    }

    pub fn get_roads_sorted_by_incoming_angle(&self, all_roads: &Vec<Road>) -> Vec<RoadID> {
        let center = self.polygon.center();
        let mut roads: Vec<RoadID> = self.roads.iter().cloned().collect();
        roads.sort_by_key(|id| {
            let r = &all_roads[id.0];
            let endpt = if r.src_i == self.id {
                r.center_pts.first_pt()
            } else if r.dst_i == self.id {
                r.center_pts.last_pt()
            } else {
                unreachable!();
            };
            endpt.angle_to(center).normalized_degrees() as i64
        });
        roads
    }

    pub fn some_outgoing_road(&self, map: &Map) -> Option<DirectedRoadID> {
        self.outgoing_lanes
            .get(0)
            .map(|l| map.get_l(*l).get_directed_parent(map))
    }

    pub fn some_incoming_road(&self, map: &Map) -> Option<DirectedRoadID> {
        self.incoming_lanes
            .get(0)
            .map(|l| map.get_l(*l).get_directed_parent(map))
    }

    pub fn name(&self, lang: Option<&String>, map: &Map) -> String {
        let road_names = self
            .roads
            .iter()
            .map(|r| map.get_r(*r).get_name(lang))
            .collect::<BTreeSet<_>>();
        abstutil::plain_list_names(road_names)
    }
}

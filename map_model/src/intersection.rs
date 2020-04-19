use crate::raw::OriginalIntersection;
use crate::{DirectedRoadID, LaneID, Map, PathConstraints, Road, RoadID, TurnID};
use geom::{Distance, Polygon};
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IntersectionID(pub usize);

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

#[derive(Serialize, Deserialize, Debug)]
pub struct Intersection {
    pub id: IntersectionID,
    // This needs to be in clockwise orientation, or later rendering of sidewalk corners breaks.
    pub polygon: Polygon,
    pub turns: BTreeSet<TurnID>,
    pub elevation: Distance,

    pub intersection_type: IntersectionType,
    pub orig_id: OriginalIntersection,

    // Note that a lane may belong to both incoming_lanes and outgoing_lanes.
    // TODO narrow down when and why. is it just sidewalks in weird cases?
    pub incoming_lanes: Vec<LaneID>,
    pub outgoing_lanes: Vec<LaneID>,

    // TODO Maybe DirectedRoadIDs
    pub roads: BTreeSet<RoadID>,
}

impl Intersection {
    pub fn is_border(&self) -> bool {
        self.intersection_type == IntersectionType::Border
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

    pub fn get_incoming_lanes(&self, map: &Map, constraints: PathConstraints) -> Vec<LaneID> {
        self.incoming_lanes
            .iter()
            .filter(|l| constraints.can_use(map.get_l(**l), map))
            .cloned()
            .collect()
    }

    // Strict for bikes. If there are bike lanes, not allowed to use other lanes.
    pub fn get_outgoing_lanes(&self, map: &Map, constraints: PathConstraints) -> Vec<LaneID> {
        let mut choices: Vec<LaneID> = self
            .outgoing_lanes
            .iter()
            .filter(|l| constraints.can_use(map.get_l(**l), map))
            .cloned()
            .collect();
        if constraints == PathConstraints::Bike {
            choices.retain(|l| map.get_l(*l).is_biking());
        }
        choices
    }

    pub fn get_zorder(&self, map: &Map) -> isize {
        // TODO Not sure min makes sense -- what about a 1 and a 0? Prefer the nonzeros. If there's
        // a -1 and a 1... need to see it to know what to do.
        self.roads
            .iter()
            .map(|r| map.get_r(*r).get_zorder())
            .min()
            .unwrap()
    }

    pub fn get_rank(&self, map: &Map) -> usize {
        self.roads
            .iter()
            .map(|r| map.get_r(*r).get_rank())
            .max()
            .unwrap()
    }

    pub(crate) fn get_roads_sorted_by_incoming_angle(&self, all_roads: &Vec<Road>) -> Vec<RoadID> {
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

    pub fn some_outgoing_road(&self, map: &Map) -> DirectedRoadID {
        map.get_l(self.outgoing_lanes[0]).get_directed_parent(map)
    }

    pub fn some_incoming_road(&self, map: &Map) -> DirectedRoadID {
        map.get_l(self.incoming_lanes[0]).get_directed_parent(map)
    }

    pub fn name(&self, map: &Map) -> String {
        let road_names = self
            .roads
            .iter()
            .map(|r| map.get_r(*r).get_name())
            .collect::<BTreeSet<_>>();
        abstutil::plain_list_names(road_names)
    }
}

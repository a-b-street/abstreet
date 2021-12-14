use std::collections::{BTreeMap, BTreeSet};
use std::convert::TryFrom;
use std::fmt;

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_usize, serialize_usize};
use geom::{Distance, Polygon};

use crate::{
    osm, CompressedMovementID, DirectedRoadID, LaneID, Map, Movement, MovementID, PathConstraints,
    Road, RoadID, RoadSideID, SideOfRoad, Turn, TurnID,
};

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
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Intersection {
    pub id: IntersectionID,
    /// This needs to be in clockwise orientation, or later rendering of sidewalk corners breaks.
    pub polygon: Polygon,
    pub turns: Vec<Turn>,
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
    // These increase the map file size, so instead, just use `recalculate_all_movements` after
    // deserializing.
    #[serde(skip_serializing, skip_deserializing)]
    pub movements: BTreeMap<MovementID, Movement>,
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

    /// Does this intersection only connect to light rail?
    pub fn is_light_rail(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| map.get_r(*r).is_light_rail())
    }

    /// Does this intersection only connect to private roads?
    pub fn is_private(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| map.get_r(*r).is_private())
    }

    /// Does this intersection only connect to footways?
    pub fn is_footway(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| map.get_r(*r).is_footway())
    }

    /// Does this intersection only connect cycleways?
    pub fn is_cycleway(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| map.get_r(*r).is_cycleway())
    }

    /// Does this intersection only connect to cycleways and footways?
    pub fn is_cycleway_or_footway(&self, map: &Map) -> bool {
        self.roads.iter().all(|r| {
            let road = map.get_r(*r);
            road.is_cycleway() || road.is_footway()
        })
    }

    /// Does this intersection only connect two road segments? Then usually, the intersection only
    /// exists to mark the road name or lanes changing.
    pub fn is_degenerate(&self) -> bool {
        self.roads.len() == 2
    }

    /// Does this intersection connect to only a single road segment?
    pub fn is_deadend(&self) -> bool {
        self.roads.len() == 1
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

    /// Higher numbers get drawn on top
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

    pub fn get_roads_sorted_by_incoming_angle(&self, map: &Map) -> Vec<RoadID> {
        let center = self.polygon.center();
        let mut roads: Vec<RoadID> = self.roads.iter().cloned().collect();
        roads.sort_by_key(|id| {
            let r = map.get_r(*id);
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

    // TODO walking_turns_v2 and the intersection geometry algorithm also do something like this.
    // Refactor?
    pub fn get_road_sides_sorted_by_incoming_angle(&self, map: &Map) -> Vec<RoadSideID> {
        let mut sides = Vec::new();
        for r in self.get_roads_sorted_by_incoming_angle(map) {
            let r = map.get_r(r);
            if r.dst_i == self.id {
                sides.push(RoadSideID {
                    road: r.id,
                    side: SideOfRoad::Right,
                });
                sides.push(RoadSideID {
                    road: r.id,
                    side: SideOfRoad::Left,
                });
            } else {
                sides.push(RoadSideID {
                    road: r.id,
                    side: SideOfRoad::Left,
                });
                sides.push(RoadSideID {
                    road: r.id,
                    side: SideOfRoad::Right,
                });
            }
        }
        sides
    }

    /// Return all incoming roads to an intersection, sorted by angle. This skips one-way roads
    /// outbound from the intersection, since no turns originate from those anyway. This allows
    /// heuristics for a 3-way intersection to not care if one of the roads happens to be a dual
    /// carriageway (split into two one-ways).
    pub fn get_sorted_incoming_roads(&self, map: &Map) -> Vec<RoadID> {
        let mut roads = Vec::new();
        for r in self.get_roads_sorted_by_incoming_angle(map) {
            if !map.get_r(r).incoming_lanes(self.id).is_empty() {
                roads.push(r);
            }
        }
        roads
    }

    pub fn some_outgoing_road(&self, map: &Map) -> Option<DirectedRoadID> {
        self.outgoing_lanes
            .get(0)
            .map(|l| map.get_l(*l).get_directed_parent())
    }

    pub fn some_incoming_road(&self, map: &Map) -> Option<DirectedRoadID> {
        self.incoming_lanes
            .get(0)
            .map(|l| map.get_l(*l).get_directed_parent())
    }

    pub fn name(&self, lang: Option<&String>, map: &Map) -> String {
        let road_names = self
            .roads
            .iter()
            .map(|r| map.get_r(*r).get_name(lang))
            .collect::<BTreeSet<_>>();
        abstutil::plain_list_names(road_names)
    }

    /// Don't call for SharedSidewalkCorners
    pub fn turn_to_movement(&self, turn: TurnID) -> (MovementID, CompressedMovementID) {
        for (idx, m) in self.movements.values().enumerate() {
            if m.members.contains(&turn) {
                return (
                    m.id,
                    CompressedMovementID {
                        i: self.id,
                        idx: u8::try_from(idx).unwrap(),
                    },
                );
            }
        }

        panic!(
            "{} doesn't belong to any movements in {} or is a SharedSidewalkCorner maybe",
            turn, self.id
        )
    }

    pub fn find_road_between<'a>(&self, other: IntersectionID, map: &'a Map) -> Option<&'a Road> {
        for r in &self.roads {
            let road = map.get_r(*r);
            if road.other_endpt(self.id) == other {
                return Some(road);
            }
        }
        None
    }
}

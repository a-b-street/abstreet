
use std::fmt;

use serde::{Deserialize, Serialize};

use geom::{Angle, PolyLine};

use crate::raw::RestrictionType;
use crate::{
    DirectedRoadID, Direction, Intersection, IntersectionID, LaneID, Map, MovementID,
    PathConstraints,
};

/// Turns are uniquely identified by their (src, dst) lanes and their parent intersection.
/// Intersection is needed to distinguish crosswalks that exist at two ends of a sidewalk.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TurnID {
    pub parent: IntersectionID,
    /// src and dst must both belong to parent. No guarantees that src is incoming and dst is
    /// outgoing for turns between sidewalks.
    pub src: LaneID,
    pub dst: LaneID,
}

impl fmt::Display for TurnID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TurnID({}, {}, {})", self.src, self.dst, self.parent)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialOrd, Ord, PartialEq, Serialize, Deserialize)]
pub enum TurnType {
    /// A marked zebra crossing, where pedestrians usually have priority
    Crosswalk,
    /// The corner where two sidewalks meet. Pedestrians can cross this without conflicting with
    /// any vehicle traffic
    SharedSidewalkCorner,
    // These are for vehicle turns
    Straight,
    Right,
    Left,
    UTurn,
    /// An unmarked crossing, where pedestrians may cross without priority over vehicles
    // TODO During the next map regeneration, sort this list to be next to crosswalk. I want to
    // avoid binary incompatibility in the meantime.
    UnmarkedCrossing,
}

impl TurnType {
    /// Is the turn a crosswalk or unmarked crossing?
    pub fn pedestrian_crossing(self) -> bool {
        self == TurnType::Crosswalk || self == TurnType::UnmarkedCrossing
    }
}

// TODO This concept may be dated, now that Movements exist. Within a movement, the lane-changing
// turns should be treated as less important.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy, PartialOrd)]
pub enum TurnPriority {
    /// For stop signs: Can't currently specify this!
    /// For traffic signals: Can't do this turn right now.
    Banned,
    /// For stop signs: cars have to stop before doing this turn, and are accepted with the lowest
    /// priority.
    /// For traffic signals: Cars can do this immediately if there are no previously accepted
    /// conflicting turns.
    Yield,
    /// For stop signs: cars can do this without stopping. These can conflict!
    /// For traffic signals: Must be non-conflicting.
    Protected,
}

/// A Turn leads from the end of one Lane to the start of another. (Except for pedestrians;
/// sidewalks are bidirectional.)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Turn {
    pub id: TurnID,
    pub turn_type: TurnType,
    // TODO Some turns might not actually have geometry. Currently encoded by two equal points.
    // Represent more directly?
    pub geom: PolyLine,
}

impl Turn {
    pub fn conflicts_with(&self, other: &Turn) -> bool {
        if self.turn_type == TurnType::SharedSidewalkCorner
            || other.turn_type == TurnType::SharedSidewalkCorner
        {
            return false;
        }
        if self.id == other.id {
            return false;
        }
        if self.between_sidewalks() && other.between_sidewalks() {
            return false;
        }

        if self.geom.first_pt() == other.geom.first_pt() {
            return false;
        }
        if self.geom.last_pt() == other.geom.last_pt() {
            return true;
        }
        self.geom.intersection(&other.geom).is_some()
    }

    // TODO What should this be for zero-length turns? Probably src's pt1 to dst's pt2 or
    // something.
    pub fn angle(&self) -> Angle {
        self.geom.first_pt().angle_to(self.geom.last_pt())
    }

    pub fn between_sidewalks(&self) -> bool {
        self.turn_type == TurnType::SharedSidewalkCorner
            || self.turn_type == TurnType::Crosswalk
            || self.turn_type == TurnType::UnmarkedCrossing
    }

    // TODO Maybe precompute this.
    /// Penalties for (lane types, lane-changing, slow lane). The penalty may depend on the vehicle
    /// performing the turn. Lower means preferable.
    pub fn penalty(&self, constraints: PathConstraints, map: &Map) -> (usize, usize, usize) {
        let from = map.get_l(self.id.src);
        let to = map.get_l(self.id.dst);

        // Starting from the farthest from the center line (right in the US), where is this travel
        // lane? Filters by the lane type and ignores lanes that don't go to the target road.
        let from_idx = {
            let mut cnt = 0;
            let r = map.get_r(from.id.road);
            for (l, lt) in r.children(from.dir).iter().rev() {
                if from.lane_type != *lt {
                    continue;
                }
                if map
                    .get_turns_from_lane(*l)
                    .into_iter()
                    .any(|t| t.id.dst.road == to.id.road)
                {
                    cnt += 1;
                    if from.id == *l {
                        break;
                    }
                }
            }
            cnt
        };

        // Starting from the farthest from the center line (right in the US), where is this travel
        // lane? Filters by the lane type.
        let to_idx = {
            let mut cnt = 0;
            let r = map.get_r(to.id.road);
            for (l, lt) in r.children(to.dir).iter().rev() {
                if to.lane_type != *lt {
                    continue;
                }
                cnt += 1;
                if to.id == *l {
                    break;
                }
            }
            cnt
        };

        // TODO I thought about different cases where there are the same/more/less lanes going in
        // and out, but then actually, I think the reasonable thing in all cases is just to do
        // this.
        let lc_cost = ((from_idx as isize) - (to_idx as isize)).abs() as usize;

        // If we're a bike, prefer bike lanes, then bus lanes. If we're a bus, prefer bus lanes.
        // Otherwise, avoid special lanes, even if we're allowed to use them sometimes because they
        // happen to double as turn lanes.
        let lt_cost = if constraints == PathConstraints::Bike {
            if to.is_biking() {
                0
            } else if to.is_bus() {
                1
            } else {
                2
            }
        } else if constraints == PathConstraints::Bus {
            if to.is_bus() {
                0
            } else {
                1
            }
        } else if to.is_bus() {
            // Cars should stay out of bus lanes unless it's required to make a turn
            3
        } else {
            0
        };

        // Keep right (in the US)
        let slow_lane = if to_idx > 1 { 1 } else { 0 };

        (lt_cost, lc_cost, slow_lane)
    }

    pub fn is_crossing_arterial_intersection(&self, map: &Map) -> bool {
        use crate::osm::RoadRank;
        if !self.turn_type.pedestrian_crossing() {
            return false;
        }
        // Distance-only metric has many false positives and negatives
        // return turn.geom.length() > Distance::feet(41.0);

        let intersection = map.get_i(self.id.parent);
        intersection.roads.iter().any(|r| {
            let rank = map.get_r(*r).get_rank();
            rank == RoadRank::Arterial || rank == RoadRank::Highway
        })
    }

    /// Is this turn legal, according to turn lane tagging?
    pub(crate) fn permitted_by_lane(&self, map: &Map) -> bool {
        if let Some(types) = map
            .get_l(self.id.src)
            .get_lane_level_turn_restrictions(map.get_parent(self.id.src), false)
        {
            types.contains(&self.turn_type)
        } else {
            true
        }
    }

    /// Is this turn legal, according to turn restrictions defined between road segments?
    pub(crate) fn permitted_by_road(&self, i: &Intersection, map: &Map) -> bool {
        if self.between_sidewalks() {
            return true;
        }

        let src = map.get_parent(self.id.src);
        let dst = self.id.dst.road;

        for (restriction, to) in &src.turn_restrictions {
            // The restriction only applies to one direction of the road.
            if !i.roads.contains(to) {
                continue;
            }
            match restriction {
                RestrictionType::BanTurns => {
                    if dst == *to {
                        return false;
                    }
                }
                RestrictionType::OnlyAllowTurns => {
                    if dst != *to {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// If this turn is a crosswalk over a single road, return that road and which end of the road
    /// is crossed.
    pub fn crosswalk_over_road(&self, map: &Map) -> Option<DirectedRoadID> {
        if !self.turn_type.pedestrian_crossing() {
            return None;
        }
        // We cross multiple roads
        if self.id.src.road != self.id.dst.road {
            return None;
        }
        Some(DirectedRoadID {
            road: self.id.src.road,
            dir: if map.get_r(self.id.src.road).dst_i == self.id.parent {
                Direction::Fwd
            } else {
                Direction::Back
            },
        })
    }
}

impl TurnID {
    pub fn to_movement(self, map: &Map) -> MovementID {
        MovementID {
            from: map.get_l(self.src).get_directed_parent(),
            to: map.get_l(self.dst).get_directed_parent(),
            parent: self.parent,
            crosswalk: map.get_l(self.src).is_walkable(),
        }
    }
}

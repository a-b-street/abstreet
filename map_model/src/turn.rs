use crate::{IntersectionID, LaneID};
use abstutil;
use geom::{Angle, PolyLine};
use serde_derive::{Deserialize, Serialize};
use std::fmt;

// Turns are uniquely identified by their (src, dst) lanes and their parent intersection.
// Intersection is needed to distinguish crosswalks that exist at two ends of a sidewalk.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TurnID {
    pub parent: IntersectionID,
    // src and dst must both belong to parent. No guarantees that src is incoming and dst is
    // outgoing for turns between sidewalks.
    pub src: LaneID,
    pub dst: LaneID,
}

impl fmt::Display for TurnID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TurnID({}, {}, {})", self.src, self.dst, self.parent)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum TurnType {
    Crosswalk,
    SharedSidewalkCorner,
    // These are for vehicle turns
    Straight,
    Right,
    Left,
}

impl TurnType {
    pub fn from_angles(from: Angle, to: Angle) -> TurnType {
        let diff = from.shortest_rotation_towards(to).normalized_degrees();
        if diff < 10.0 || diff > 350.0 {
            TurnType::Straight
        } else if diff > 180.0 {
            // Clockwise rotation
            TurnType::Right
        } else {
            // Counter-clockwise rotation
            TurnType::Left
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Copy, PartialOrd)]
pub enum TurnPriority {
    // Can't do this turn at all!
    Banned,
    // For stop signs: cars have to stop before doing this turn, and are accepted with the lowest priority.
    // For traffic signals: this priority doesn't make sense; can't be used.
    Stop,
    // Cars can do this immediately if there are no previously accepted conflicting turns.
    Yield,
    // These must be non-conflicting, and cars don't have to stop before doing this turn (unless a
    // conflicting Yield has been accepted).
    // TODO Rename Protected?
    Priority,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Turn {
    pub id: TurnID,
    pub turn_type: TurnType,
    // TODO Some turns might not actually have geometry. Currently encoded by two equal points.
    // Represent more directly?
    pub geom: PolyLine,

    // Just for convenient debugging lookup.
    pub lookup_idx: usize,
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
        self.turn_type == TurnType::SharedSidewalkCorner || self.turn_type == TurnType::Crosswalk
    }
    pub fn other_crosswalk_id(&self) -> TurnID {
        assert_eq!(self.turn_type, TurnType::Crosswalk);
        TurnID {
            parent: self.id.parent,
            src: self.id.dst,
            dst: self.id.src,
        }
    }

    pub fn dump_debug(&self) {
        println!("{}", abstutil::to_json(self));
    }
}

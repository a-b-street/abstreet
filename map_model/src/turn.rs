// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use geom::{Angle, Line, Pt2D};
use std::f64;
use std::fmt;
use {IntersectionID, LaneID, Map};

// Turns are uniquely identified by their (src, dst) lanes and their parent intersection.
// Intersection is needed to distinguish crosswalks that exist at two ends of a sidewalk.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TurnID {
    pub parent: IntersectionID,
    pub src: LaneID,
    pub dst: LaneID,
}

impl fmt::Display for TurnID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TurnID({0}, {1}, {2})", self.parent, self.src, self.dst)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Turn {
    // parent, src, dst are all encoded by id. TODO dedupe.
    pub id: TurnID,
    // src and dst must both belong to parent. No guarantees that src is incoming and dst is
    // outgoing for turns between sidewalks.
    pub parent: IntersectionID,
    pub src: LaneID,
    pub dst: LaneID,
    pub between_sidewalks: bool,

    // TODO Really tempted to be either a Line or a Pt2D, to emphasize that this might not actually
    // cover any space
    pub line: Line,
}

impl PartialEq for Turn {
    fn eq(&self, other: &Turn) -> bool {
        self.id == other.id
    }
}

impl Turn {
    pub fn conflicts_with(&self, other: &Turn) -> bool {
        if self == other {
            return false;
        }
        if self.between_sidewalks && other.between_sidewalks {
            return false;
        }

        if self.line.pt1() == other.line.pt1() {
            return false;
        }
        if self.line.pt2() == other.line.pt2() {
            return true;
        }
        self.line.intersects(&other.line)
    }

    // TODO share impl with GeomLane
    pub fn dist_along(&self, dist_along: si::Meter<f64>) -> (Pt2D, Angle) {
        (self.line.dist_along(dist_along), self.line.angle())
    }

    pub fn length(&self) -> si::Meter<f64> {
        self.line.length()
    }

    // TODO all the stuff based on turn angle is a bit... wrong, especially for sidewalks. :\
    // also, make sure right/left/straight are disjoint... and maybe cover all turns. return an enum from one method.
    pub fn turn_angle(&self, map: &Map) -> Angle {
        let lane_angle = map.get_l(self.src).end_line(self.parent).angle();
        self.line.angle() - lane_angle
    }

    pub fn is_right_turn(&self, map: &Map) -> bool {
        let a = self.turn_angle(map).normalized_degrees();
        a < 95.0 && a > 20.0
    }

    pub fn is_straight_turn(&self, map: &Map) -> bool {
        let a = self.turn_angle(map).normalized_degrees();
        a <= 20.0 || a >= 320.0
    }
}

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use geom::{Angle, Line, Pt2D};
use std::f64;
use std::fmt;
use IntersectionID;
use LaneID;

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

#[derive(Debug)]
pub struct Turn {
    // parent, src, dst are all encoded by id. TODO dedupe.
    pub id: TurnID,
    // src and dst must both belong to parent. No guarantees that src is incoming and dst is
    // outgoing for turns between sidewalks.
    pub parent: IntersectionID,
    pub src: LaneID,
    pub dst: LaneID,
    pub between_sidewalks: bool,

    /// GeomTurn stuff
    pub line: Line,
}

impl PartialEq for Turn {
    fn eq(&self, other: &Turn) -> bool {
        self.id == other.id
    }
}

impl Turn {
    pub fn conflicts_with(&self, other: &Turn) -> bool {
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
}

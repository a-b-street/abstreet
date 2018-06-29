// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use Angle;
use IntersectionID;
use Line;
use Pt2D;
use RoadID;
use dimensioned::si;
use std::f64;

// TODO reconsider pub usize. maybe outside world shouldnt know.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct TurnID(pub usize);

#[derive(Debug)]
pub struct Turn {
    pub id: TurnID,
    pub parent: IntersectionID,
    pub src: RoadID,
    pub dst: RoadID,

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
        if self.line.pt1() == other.line.pt1() {
            return false;
        }
        if self.line.pt2() == other.line.pt2() {
            return true;
        }
        self.line.intersects(&other.line)
    }

    // TODO share impl with GeomRoad
    pub fn dist_along(&self, dist_along: si::Meter<f64>) -> (Pt2D, Angle) {
        (self.line.dist_along(dist_along), self.line.angle())
    }

    pub fn length(&self) -> si::Meter<f64> {
        self.line.length()
    }
}

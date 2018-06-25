// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use IntersectionID;
use Pt2D;
use RoadID;
use dimensioned::si;
use geometry;
use graphics::math::Vec2d;
use std::f64;
use vecmath;

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
    pub(crate) src_pt: Vec2d,
    pub dst_pt: Vec2d,
}

impl PartialEq for Turn {
    fn eq(&self, other: &Turn) -> bool {
        self.id == other.id
    }
}

impl Turn {
    pub fn conflicts_with(&self, other: &Turn) -> bool {
        if self.src_pt == other.src_pt {
            return false;
        }
        if self.dst_pt == other.dst_pt {
            return true;
        }
        geometry::line_segments_intersect(
            (&self.src_pt, &self.dst_pt),
            (&other.src_pt, &other.dst_pt),
        )
    }

    // TODO share impl with GeomRoad
    pub fn dist_along(&self, dist_along: si::Meter<f64>) -> (Pt2D, geometry::angles::Radian<f64>) {
        let src = Pt2D::new(self.src_pt[0], self.src_pt[1]);
        let dst = Pt2D::new(self.dst_pt[0], self.dst_pt[1]);
        let vec = geometry::safe_dist_along_line((&src, &dst), dist_along);
        (Pt2D::new(vec[0], vec[1]), geometry::angle(&src, &dst))
    }

    pub fn length(&self) -> si::Meter<f64> {
        let src = Pt2D::new(self.src_pt[0], self.src_pt[1]);
        let dst = Pt2D::new(self.dst_pt[0], self.dst_pt[1]);
        geometry::euclid_dist((&src, &dst))
    }

    pub fn slope(&self) -> [f64; 2] {
        vecmath::vec2_normalized([
            self.dst_pt[0] - self.src_pt[0],
            self.dst_pt[1] - self.src_pt[1],
        ])
    }
}

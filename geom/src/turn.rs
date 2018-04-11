// Copyright 2018 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

extern crate dimensioned;
extern crate map_model;

use dimensioned::si;
use geometry;
use graphics::math::Vec2d;
use map_model::{Pt2D, TurnID};
use road::GeomRoad;
use std::f64;
use vecmath;

#[derive(Debug)]
pub struct GeomTurn {
    pub id: TurnID,
    src_pt: Vec2d,
    pub dst_pt: Vec2d,
}

impl GeomTurn {
    pub fn new(roads: &[GeomRoad], turn: &map_model::Turn) -> GeomTurn {
        let src_pt = roads[turn.src.0].last_pt();
        let dst_pt = roads[turn.dst.0].first_pt();

        GeomTurn {
            id: turn.id,
            src_pt,
            dst_pt,
        }
    }

    pub fn conflicts_with(&self, other: &GeomTurn) -> bool {
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

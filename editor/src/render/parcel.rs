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

extern crate aabb_quadtree;
extern crate map_model;

use aabb_quadtree::geom::Rect;
use ezgui::canvas::GfxCtx;
use geom::geometry;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::{Bounds, ParcelID, Pt2D};
use render::PARCEL_BOUNDARY_THICKNESS;

#[derive(Debug)]
pub struct DrawParcel {
    pub id: ParcelID,
    polygons: Vec<Vec<Vec2d>>,
}

impl DrawParcel {
    pub fn new(p: &map_model::Parcel, bounds: &Bounds) -> DrawParcel {
        let pts: Vec<Pt2D> = p.points
            .iter()
            .map(|pt| geometry::gps_to_screen_space(pt, bounds))
            .collect();
        DrawParcel {
            id: p.id,
            polygons: geometry::thick_multiline(
                &geometry::ThickLine::Centered(PARCEL_BOUNDARY_THICKNESS),
                &pts,
            ),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        for p in &self.polygons {
            poly.draw(p, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }
    }

    //pub fn contains_pt(&self, x: f64, y: f64) -> bool {}

    pub fn get_bbox(&self) -> Rect {
        geometry::get_bbox_for_polygons(&self.polygons)
    }
}

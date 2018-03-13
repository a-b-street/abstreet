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
use ezgui::canvas;
use ezgui::canvas::GfxCtx;
use geometry;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::{Bounds, IntersectionID, Map};
use render::DrawRoad;
use std::f64;
use svg;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub point: Vec2d,

    polygon: Vec<Vec2d>,
}

impl DrawIntersection {
    pub fn new(
        inter: &map_model::Intersection,
        map: &Map,
        roads: &Vec<DrawRoad>,
        bounds: &Bounds,
    ) -> DrawIntersection {
        let mut pts: Vec<Vec2d> = Vec::new();
        // TODO this smashes encapsulation to bits :D
        for r in &map.get_roads_to_intersection(inter.id) {
            let dr = &roads[r.id.0];
            pts.push(dr.polygons.last().unwrap()[2]);
            pts.push(dr.polygons.last().unwrap()[3]);
        }
        for r in &map.get_roads_from_intersection(inter.id) {
            let dr = &roads[r.id.0];
            pts.push(dr.polygons[0][0]);
            pts.push(dr.polygons[0][1]);
        }

        let center = geometry::gps_to_screen_space(&inter.point, bounds);
        // Sort points by angle from the center
        pts.sort_by_key(|pt| {
            let mut angle = (pt[1] - center.y()).atan2(pt[0] - center.x()).to_degrees();
            if angle < 0.0 {
                angle += 360.0;
            }
            angle as i64
        });
        let first_pt = pts[0].clone();
        pts.push(first_pt);

        DrawIntersection {
            id: inter.id,
            point: [center.x(), center.y()],
            polygon: pts,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        poly.draw(&self.polygon, &g.ctx.draw_state, g.ctx.transform, g.gfx);
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.polygon)
    }

    pub fn get_bbox(&self) -> Rect {
        geometry::get_bbox_for_polygons(&[self.polygon.clone()])
    }

    pub fn to_svg(&self, doc: svg::Document, color: Color) -> svg::Document {
        let mut data = svg::node::element::path::Data::new();
        data = data.move_to((self.polygon[0][0], self.polygon[0][1]));
        for pt in self.polygon.iter().skip(1) {
            data = data.line_to((pt[0], pt[1]));
        }
        let path = svg::node::element::Path::new()
            .set("fill", canvas::color_to_svg(color))
            .set("d", data);
        doc.add(path)
    }
}

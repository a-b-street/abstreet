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
use geom::geometry;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::{Bounds, BuildingID};
use std::f64;
use svg;

#[derive(Debug)]
pub struct DrawBuilding {
    pub id: BuildingID,
    polygon: Vec<Vec2d>,
}

impl DrawBuilding {
    pub fn new(bldg: &map_model::Building, bounds: &Bounds) -> DrawBuilding {
        DrawBuilding {
            id: bldg.id,
            polygon: bldg.points
                .iter()
                .map(|pt| {
                    let screen_pt = geometry::gps_to_screen_space(pt, bounds);
                    [screen_pt.x(), screen_pt.y()]
                })
                .collect(),
        }
    }

    // TODO it'd be cool to draw a thick border. how to expand a polygon?
    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        poly.draw(&self.polygon, &g.ctx.draw_state, g.ctx.transform, g.gfx);
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.polygon)
    }

    pub fn tooltip_lines(&self, map: &map_model::Map) -> Vec<String> {
        let b = map.get_b(self.id);
        let mut lines = vec![
            format!("Building #{:?} (from OSM way {})", self.id, b.osm_way_id),
        ];
        lines.extend(b.osm_tags.iter().cloned());
        lines
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

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
use geom;
use geom::geometry;
use geom::GeomMap;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::TurnID;
use render::{BIG_ARROW_TIP_LENGTH, TURN_ICON_ARROW_LENGTH, TURN_ICON_ARROW_THICKNESS,
             TURN_ICON_ARROW_TIP_LENGTH, TURN_ICON_CIRCLE_COLOR};
use std::f64;
use svg;
use vecmath;

#[derive(Debug)]
pub struct DrawTurn {
    pub id: TurnID,
    src_pt: Vec2d,
    pub dst_pt: Vec2d,
    icon_circle: [f64; 4],
    icon_arrow: [f64; 4],
}

impl DrawTurn {
    pub fn new(geom_map: &GeomMap, turn: &map_model::Turn, offset_along_road: usize) -> DrawTurn {
        let offset_along_road = offset_along_road as f64;
        let src_pt = geom_map.get_r(turn.src).last_pt();
        let dst_pt = geom_map.get_r(turn.dst).first_pt();
        let slope = vecmath::vec2_normalized([dst_pt[0] - src_pt[0], dst_pt[1] - src_pt[1]]);
        let last_line = geom_map.get_r(turn.src).last_line();

        let icon_center = geometry::dist_along_line(
            // Start the distance from the intersection
            (&last_line.1, &last_line.0),
            (offset_along_road + 0.5) * TURN_ICON_ARROW_LENGTH,
        );
        let icon_src = [
            icon_center[0] - (TURN_ICON_ARROW_LENGTH / 2.0) * slope[0],
            icon_center[1] - (TURN_ICON_ARROW_LENGTH / 2.0) * slope[1],
        ];
        let icon_dst = [
            icon_center[0] + (TURN_ICON_ARROW_LENGTH / 2.0) * slope[0],
            icon_center[1] + (TURN_ICON_ARROW_LENGTH / 2.0) * slope[1],
        ];

        let icon_circle =
            geometry::circle(icon_center[0], icon_center[1], TURN_ICON_ARROW_LENGTH / 2.0);

        let icon_arrow = [icon_src[0], icon_src[1], icon_dst[0], icon_dst[1]];

        DrawTurn {
            id: turn.id,
            src_pt,
            dst_pt,
            icon_circle,
            icon_arrow,
        }
    }

    pub fn draw_full(&self, g: &mut GfxCtx, color: Color) {
        let turn_line = graphics::Line::new_round(color, geom::BIG_ARROW_THICKNESS);
        turn_line.draw_arrow(
            [
                self.src_pt[0],
                self.src_pt[1],
                self.dst_pt[0],
                self.dst_pt[1],
            ],
            BIG_ARROW_TIP_LENGTH,
            &g.ctx.draw_state,
            g.ctx.transform,
            g.gfx,
        );
    }

    pub fn draw_icon(&self, g: &mut GfxCtx, color: Color) {
        let circle = graphics::Ellipse::new(TURN_ICON_CIRCLE_COLOR);
        circle.draw(self.icon_circle, &g.ctx.draw_state, g.ctx.transform, g.gfx);

        let turn_line = graphics::Line::new_round(color, TURN_ICON_ARROW_THICKNESS);
        turn_line.draw_arrow(
            self.icon_arrow,
            TURN_ICON_ARROW_TIP_LENGTH,
            &g.ctx.draw_state,
            g.ctx.transform,
            g.gfx,
        );
    }

    // the two below are for the icon
    pub fn get_bbox(&self) -> Rect {
        geometry::circle_to_bbox(&self.icon_circle)
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        let radius = self.icon_circle[2] / 2.0;
        geometry::point_in_circle(
            x,
            y,
            [self.icon_circle[0] + radius, self.icon_circle[1] + radius],
            radius,
        )
    }

    pub fn to_svg(&self, doc: svg::Document, _color: Color) -> svg::Document {
        doc
    }
}

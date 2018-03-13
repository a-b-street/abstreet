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
use geometry;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use render::{BIG_ARROW_THICKNESS, BRIGHT_DEBUG_COLOR, DEBUG_COLOR, LANE_THICKNESS,
             PARCEL_BOUNDARY_THICKNESS, ROAD_ORIENTATION_COLOR, TURN_DIST_FROM_INTERSECTION,
             TURN_ICON_ARROW_LENGTH};
use map_model::{Bounds, Pt2D, RoadID};
use std::f64;
use svg;

#[derive(Debug)]
pub struct DrawRoad {
    pub id: RoadID,
    pub polygons: Vec<Vec<Vec2d>>, // TODO pub for DrawIntersection
    // Empty for one-ways and one side of two-ways.
    // TODO ideally this could be done in the shader or something
    yellow_center_lines: Vec<Pt2D>,
    // A circle to represent the end of the road, where it meets the intersection. Only there for
    // intersections controlled by stop sign.
    end_icon_circle: Option<[f64; 4]>,
    // TODO need to settle on a proper Line type
    pub lane_center_lines: Vec<(Pt2D, Pt2D)>,
}

impl DrawRoad {
    pub fn new(road: &map_model::Road, bounds: &Bounds, leads_to_stop_sign: bool) -> DrawRoad {
        let mut pts: Vec<Pt2D> = road.points
            .iter()
            .map(|pt| geometry::gps_to_screen_space(pt, bounds))
            .collect();
        // Shove the lines away from the intersection so they don't overlap.
        // TODO deal with tiny roads
        let num_pts = pts.len();
        let new_first_pt =
            geometry::dist_along_line((&pts[0], &pts[1]), TURN_DIST_FROM_INTERSECTION);
        let new_last_pt = geometry::dist_along_line(
            (&pts[num_pts - 1], &pts[num_pts - 2]),
            TURN_DIST_FROM_INTERSECTION,
        );
        pts[0] = Pt2D::from(new_first_pt);
        pts[num_pts - 1] = Pt2D::from(new_last_pt);

        let use_yellow_center_lines = if let Some(other) = road.other_side {
            road.id.0 < other.0
        } else {
            false
        };

        let lane_center_shift = if road.other_side.is_none() {
            0.0
        } else if use_yellow_center_lines {
            // TODO I think this is unfair to one side, right? If we hover over the yellow line, it
            // shouldn't match either lane. Needs to be its own thing, or adjust the bbox.
            (LANE_THICKNESS / 2.0) + (BIG_ARROW_THICKNESS / 2.0)
        } else {
            (LANE_THICKNESS / 2.0)
        };
        let lane_center_lines: Vec<(Pt2D, Pt2D)> = pts.windows(2)
            .map(|pair| {
                geometry::shift_line_perpendicularly_in_driving_direction(
                    lane_center_shift,
                    &pair[0],
                    &pair[1],
                )
            })
            .collect();

        let thick_line = if road.other_side.is_some() {
            geometry::ThickLine::DrivingDirectionOnly(LANE_THICKNESS)
        } else {
            geometry::ThickLine::Centered(LANE_THICKNESS)
        };

        let end_icon_center = geometry::dist_along_line(
            (
                &lane_center_lines[lane_center_lines.len() - 1].1,
                &lane_center_lines[lane_center_lines.len() - 1].0,
            ),
            0.5 * TURN_ICON_ARROW_LENGTH,
        );

        DrawRoad {
            lane_center_lines,
            id: road.id,
            polygons: geometry::thick_multiline(&thick_line, &pts),
            yellow_center_lines: if use_yellow_center_lines {
                pts
            } else {
                Vec::new()
            },
            end_icon_circle: if leads_to_stop_sign {
                Some(geometry::circle(
                    end_icon_center[0],
                    end_icon_center[1],
                    TURN_ICON_ARROW_LENGTH / 2.0,
                ))
            } else {
                None
            },
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        for p in &self.polygons {
            poly.draw(p, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }
    }

    pub fn draw_detail(&self, g: &mut GfxCtx) {
        let road_marking = graphics::Line::new_round(ROAD_ORIENTATION_COLOR, BIG_ARROW_THICKNESS);

        for pair in self.yellow_center_lines.windows(2) {
            road_marking.draw(
                [pair[0].x(), pair[0].y(), pair[1].x(), pair[1].y()],
                &g.ctx.draw_state,
                g.ctx.transform,
                g.gfx,
            );
        }
    }

    pub fn draw_debug(&self, g: &mut GfxCtx) {
        let line = graphics::Line::new_round(DEBUG_COLOR, PARCEL_BOUNDARY_THICKNESS / 2.0);
        let circle = graphics::Ellipse::new(BRIGHT_DEBUG_COLOR);

        for &(pt1, pt2) in &self.lane_center_lines {
            line.draw(
                [pt1.x(), pt1.y(), pt2.x(), pt2.y()],
                &g.ctx.draw_state,
                g.ctx.transform,
                g.gfx,
            );
            circle.draw(
                geometry::circle(pt1.x(), pt1.y(), 0.4),
                &g.ctx.draw_state,
                g.ctx.transform,
                g.gfx,
            );
            circle.draw(
                geometry::circle(pt2.x(), pt2.y(), 0.8),
                &g.ctx.draw_state,
                g.ctx.transform,
                g.gfx,
            );
        }
    }

    pub fn draw_icon(&self, g: &mut GfxCtx, color: Color) {
        assert!(self.end_icon_circle.is_some());
        let circle = graphics::Ellipse::new(color);
        circle.draw(
            self.end_icon_circle.unwrap(),
            &g.ctx.draw_state,
            g.ctx.transform,
            g.gfx,
        );
    }

    pub fn get_bbox_for_road(&self) -> Rect {
        geometry::get_bbox_for_polygons(&self.polygons)
    }

    pub fn get_bbox_for_icon(&self) -> Option<Rect> {
        self.end_icon_circle.as_ref().map(geometry::circle_to_bbox)
    }

    pub fn road_contains_pt(&self, x: f64, y: f64) -> bool {
        for p in &self.polygons {
            if geometry::point_in_polygon(x, y, p) {
                return true;
            }
        }
        false
    }

    pub fn icon_contains_pt(&self, x: f64, y: f64) -> bool {
        assert!(self.end_icon_circle.is_some());
        let circle = self.end_icon_circle.unwrap();

        let radius = circle[2] / 2.0;
        geometry::point_in_circle(x, y, [circle[0] + radius, circle[1] + radius], radius)
    }

    pub fn tooltip_lines(&self, map: &map_model::Map) -> Vec<String> {
        let r = map.get_r(self.id);
        // TODO length in meters
        let mut lines = vec![
            format!(
                "Road #{:?} (from OSM way {}) has {} polygons",
                self.id,
                r.osm_way_id,
                self.polygons.len()
            ),
            format!(
                "Road goes from {}m to {}m",
                map.get_source_intersection(self.id).elevation_meters,
                map.get_destination_intersection(self.id).elevation_meters,
            ),
            format!("Road is {}m long", self.length()),
        ];
        lines.extend(r.osm_tags.iter().cloned());
        lines
    }

    pub fn to_svg(
        &self,
        doc: svg::Document,
        _road_color: Color,
        _icon_color: Color,
    ) -> svg::Document {
        doc
    }

    // TODO these don't really fit here

    pub fn first_pt(&self) -> Vec2d {
        let pt = &self.lane_center_lines[0].0;
        [pt.x(), pt.y()]
    }

    pub fn last_pt(&self) -> Vec2d {
        let pt = &self.lane_center_lines[self.lane_center_lines.len() - 1].1;
        [pt.x(), pt.y()]
    }

    pub fn last_line(&self) -> (Pt2D, Pt2D) {
        self.lane_center_lines[self.lane_center_lines.len() - 1]
    }

    pub fn dist_along(&self, dist_along: f64) -> (Pt2D, f64) {
        // TODO valid to do euclidean distance on screen-space points that're formed from
        // Haversine?
        let mut dist_left = dist_along;
        for l in &self.lane_center_lines {
            let length = geometry::euclid_dist((&l.0, &l.1));
            if dist_left < length {
                let vec = geometry::safe_dist_along_line((&l.0, &l.1), dist_left);
                let angle = (l.1.y() - l.0.y()).atan2(l.1.x() - l.0.x());
                return (Pt2D::new(vec[0], vec[1]), angle);
            }
            dist_left -= length;
        }
        panic!("{} is longer than road {:?}", dist_along, self.id);
    }

    pub fn length(&self) -> f64 {
        self.lane_center_lines.iter().fold(0.0, |so_far, l| {
            so_far + geometry::euclid_dist((&l.0, &l.1))
        })
    }
}

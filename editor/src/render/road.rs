// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate map_model;

use aabb_quadtree::geom::Rect;
use ezgui::canvas::GfxCtx;
use geom;
use geom::geometry;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::{Pt2D, RoadID};
use render::{BRIGHT_DEBUG_COLOR, DEBUG_COLOR, PARCEL_BOUNDARY_THICKNESS, ROAD_ORIENTATION_COLOR};
use std::f64;

#[derive(Debug)]
pub struct DrawRoad {
    pub id: RoadID,
    pub polygons: Vec<Vec<Vec2d>>, // TODO pub for DrawIntersection
    // Empty for one-ways and one side of two-ways.
    // TODO ideally this could be done in the shader or something
    yellow_center_lines: Vec<Pt2D>,
}

impl DrawRoad {
    pub fn new(road: &map_model::Road, geom_map: &geom::GeomMap) -> DrawRoad {
        let geom_r = geom_map.get_r(road.id);

        let use_yellow_center_lines = if let Some(other) = road.other_side {
            road.id.0 < other.0
        } else {
            false
        };

        let thick_line = if road.other_side.is_some() {
            geometry::ThickLine::DrivingDirectionOnly(geom::LANE_THICKNESS)
        } else {
            geometry::ThickLine::Centered(geom::LANE_THICKNESS)
        };

        DrawRoad {
            id: road.id,
            polygons: geometry::thick_multiline(&thick_line, &geom_r.pts),
            yellow_center_lines: if use_yellow_center_lines {
                geom_r.pts.clone()
            } else {
                Vec::new()
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
        let road_marking =
            graphics::Line::new_round(ROAD_ORIENTATION_COLOR, geom::BIG_ARROW_THICKNESS);

        for pair in self.yellow_center_lines.windows(2) {
            road_marking.draw(
                [pair[0].x(), pair[0].y(), pair[1].x(), pair[1].y()],
                &g.ctx.draw_state,
                g.ctx.transform,
                g.gfx,
            );
        }
    }

    pub fn draw_debug(&self, g: &mut GfxCtx, geom_r: &geom::GeomRoad) {
        let line = graphics::Line::new_round(DEBUG_COLOR, PARCEL_BOUNDARY_THICKNESS / 2.0);
        let circle = graphics::Ellipse::new(BRIGHT_DEBUG_COLOR);

        for &(pt1, pt2) in &geom_r.lane_center_lines {
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

    pub fn get_bbox_for_road(&self) -> Rect {
        geometry::get_bbox_for_polygons(&self.polygons)
    }

    pub fn road_contains_pt(&self, x: f64, y: f64) -> bool {
        for p in &self.polygons {
            if geometry::point_in_polygon(x, y, p) {
                return true;
            }
        }
        false
    }

    pub fn tooltip_lines(&self, map: &map_model::Map, geom_map: &geom::GeomMap) -> Vec<String> {
        let r = map.get_r(self.id);
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
            format!("Road is {}m long", geom_map.get_r(self.id).length()),
        ];
        lines.extend(r.osm_tags.iter().cloned());
        lines
    }

    // Get the line marking the end of the road, perpendicular to the direction of the road
    pub(crate) fn get_end_crossing(&self) -> (Vec2d, Vec2d) {
        (
            self.polygons.last().unwrap()[2],
            self.polygons.last().unwrap()[3],
        )
    }

    pub(crate) fn get_start_crossing(&self) -> (Vec2d, Vec2d) {
        (self.polygons[0][0], self.polygons[0][1])
    }
}

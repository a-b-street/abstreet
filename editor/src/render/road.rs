// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::{ColorScheme, Colors};
use ezgui::GfxCtx;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model;
use map_model::geometry;
use map_model::{Pt2D, RoadID};
use render::PARCEL_BOUNDARY_THICKNESS;
use std::f64;

#[derive(Debug)]
pub struct DrawRoad {
    pub id: RoadID,
    polygons: Vec<Vec<Vec2d>>,
    // Empty for one-ways and one side of two-ways.
    // TODO ideally this could be done in the shader or something
    yellow_center_lines: Vec<Pt2D>,
    start_crossing: (Vec2d, Vec2d),
    end_crossing: (Vec2d, Vec2d),
}

impl DrawRoad {
    pub fn new(road: &map_model::Road) -> DrawRoad {
        let thick_line = geometry::ThickLine::Centered(geometry::LANE_THICKNESS);
        let lane_center_pts: Vec<Pt2D> = road.lane_center_lines
            .iter()
            .flat_map(|pair| vec![pair.0, pair.1])
            .collect();

        let (first1, first2) = road.lane_center_lines[0];
        let (start_1, _) = map_model::shift_line(geometry::LANE_THICKNESS / 2.0, first1, first2);
        let (_, start_2) = map_model::shift_line(geometry::LANE_THICKNESS / 2.0, first2, first1);

        let (last1, last2) = *road.lane_center_lines.last().unwrap();
        let (_, end_1) = map_model::shift_line(geometry::LANE_THICKNESS / 2.0, last1, last2);
        let (end_2, _) = map_model::shift_line(geometry::LANE_THICKNESS / 2.0, last2, last1);

        DrawRoad {
            id: road.id,
            polygons: geometry::thick_multiline(&thick_line, &lane_center_pts),
            //polygons: map_model::polygons_for_polyline(&lane_center_pts, geometry::LANE_THICKNESS),
            yellow_center_lines: if road.use_yellow_center_lines {
                road.unshifted_pts.clone()
            } else {
                Vec::new()
            },
            start_crossing: (start_1.to_vec(), start_2.to_vec()),
            end_crossing: (end_1.to_vec(), end_2.to_vec()),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        for p in &self.polygons {
            poly.draw(p, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }
    }

    pub fn draw_detail(&self, g: &mut GfxCtx, cs: &ColorScheme) {
        let road_marking = graphics::Line::new_round(
            cs.get(Colors::RoadOrientation),
            geometry::BIG_ARROW_THICKNESS,
        );

        for pair in self.yellow_center_lines.windows(2) {
            road_marking.draw(
                [pair[0].x(), pair[0].y(), pair[1].x(), pair[1].y()],
                &g.ctx.draw_state,
                g.ctx.transform,
                g.gfx,
            );
        }
    }

    pub fn draw_debug(&self, g: &mut GfxCtx, cs: &ColorScheme, r: &map_model::Road) {
        let line =
            graphics::Line::new_round(cs.get(Colors::Debug), PARCEL_BOUNDARY_THICKNESS / 2.0);
        let circle = graphics::Ellipse::new(cs.get(Colors::BrightDebug));

        for &(pt1, pt2) in &r.lane_center_lines {
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

    pub fn tooltip_lines(&self, map: &map_model::Map) -> Vec<String> {
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
            format!("Road is {}m long", r.length()),
        ];
        lines.extend(r.osm_tags.iter().cloned());
        lines
    }

    // Get the line marking the end of the road, perpendicular to the direction of the road
    pub(crate) fn get_end_crossing(&self) -> (Vec2d, Vec2d) {
        self.end_crossing
    }

    pub(crate) fn get_start_crossing(&self) -> (Vec2d, Vec2d) {
        self.start_crossing
    }
}

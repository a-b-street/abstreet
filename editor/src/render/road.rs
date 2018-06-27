// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::{ColorScheme, Colors};
use dimensioned::si;
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

    marking_lines: Vec<(Vec2d, Vec2d)>,
    // Remember so we know how to color marking_lines
    lane_type: map_model::LaneType,
}

impl DrawRoad {
    pub fn new(road: &map_model::Road) -> DrawRoad {
        let (first1, first2) = road.first_line();
        let (start_1, start_2) = perp_line(first1, first2, geometry::LANE_THICKNESS);

        let (last1, last2) = road.last_line();
        let (end_1, end_2) = perp_line(last2, last1, geometry::LANE_THICKNESS);

        let polygons =
            map_model::polygons_for_polyline(&road.lane_center_pts, geometry::LANE_THICKNESS);

        DrawRoad {
            id: road.id,
            polygons,
            yellow_center_lines: if road.use_yellow_center_lines {
                road.unshifted_pts.clone()
            } else {
                Vec::new()
            },
            start_crossing: (start_1, start_2),
            end_crossing: (end_1, end_2),
            marking_lines: match road.lane_type {
                map_model::LaneType::Sidewalk => calculate_sidewalk_lines(road),
                map_model::LaneType::Parking => calculate_parking_lines(road),
                map_model::LaneType::Driving => calculate_driving_lines(road),
            },
            lane_type: road.lane_type,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        for p in &self.polygons {
            poly.draw(p, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }
    }

    pub fn draw_detail(&self, g: &mut GfxCtx, cs: &ColorScheme) {
        let center_marking = graphics::Line::new_round(
            cs.get(Colors::RoadOrientation),
            geometry::BIG_ARROW_THICKNESS,
        );

        for pair in self.yellow_center_lines.windows(2) {
            center_marking.draw(
                [pair[0].x(), pair[0].y(), pair[1].x(), pair[1].y()],
                &g.ctx.draw_state,
                g.ctx.transform,
                g.gfx,
            );
        }

        let extra_marking_color = match self.lane_type {
            map_model::LaneType::Sidewalk => cs.get(Colors::SidewalkMarking),
            map_model::LaneType::Parking => cs.get(Colors::ParkingMarking),
            map_model::LaneType::Driving => cs.get(Colors::DrivingLaneMarking),
        };
        let extra_marking = graphics::Line::new(
            extra_marking_color,
            // TODO move this somewhere
            0.25,
        );
        for pair in &self.marking_lines {
            extra_marking.draw(
                [pair.0[0], pair.0[1], pair.1[0], pair.1[1]],
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

        for pair in r.lane_center_pts.windows(2) {
            let (pt1, pt2) = (pair[0], pair[1]);
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
                "Road goes from {} to {}",
                map.get_source_intersection(self.id).elevation,
                map.get_destination_intersection(self.id).elevation,
            ),
            format!("Road is {}m long", r.length()),
        ];
        for (k, v) in &r.osm_tags {
            lines.push(format!("{} = {}", k, v));
        }
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

// TODO this always does it at pt1
fn perp_line(orig1: Pt2D, orig2: Pt2D, length: f64) -> (Vec2d, Vec2d) {
    let (pt1, _) = map_model::shift_line(length / 2.0, orig1, orig2);
    let (_, pt2) = map_model::shift_line(length / 2.0, orig2, orig1);
    (pt1.to_vec(), pt2.to_vec())
}

fn calculate_sidewalk_lines(road: &map_model::Road) -> Vec<(Vec2d, Vec2d)> {
    let tile_every = geometry::LANE_THICKNESS * si::M;

    let length = road.length();

    let mut result = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, angle) = road.dist_along(dist_along);
        // Reuse perp_line. Project away an arbitrary amount
        let pt2 = Pt2D::new(
            pt.x() + angle.value_unsafe.cos(),
            pt.y() + angle.value_unsafe.sin(),
        );
        result.push(perp_line(pt, pt2, geometry::LANE_THICKNESS));
        dist_along += tile_every;
    }

    result
}

fn calculate_parking_lines(road: &map_model::Road) -> Vec<(Vec2d, Vec2d)> {
    // TODO look up this value
    let tile_every = 10.0 * si::M;
    // meters, but the dims get annoying below to remove
    // TODO make Pt2D natively understand meters, projecting away by an angle
    let leg_length = 1.0;
    let pi = f64::consts::PI;

    let length = road.length();

    let mut result = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, lane_angle) = road.dist_along(dist_along);
        let perp_angle = lane_angle.value_unsafe + (pi * 1.5);
        // Find the outside of the lane. Actually, shift inside a little bit, since the line will
        // have thickness, but shouldn't really intersect the adjacent line when drawn.
        let t_pt = Pt2D::new(
            pt.x() + (geometry::LANE_THICKNESS * 0.4) * perp_angle.cos(),
            pt.y() + (geometry::LANE_THICKNESS * 0.4) * perp_angle.sin(),
        );
        // The perp leg
        result.push((
            [t_pt.x(), t_pt.y()],
            [
                t_pt.x() - leg_length * perp_angle.cos(),
                t_pt.y() - leg_length * perp_angle.sin(),
            ],
        ));
        // Upper leg
        result.push((
            [t_pt.x(), t_pt.y()],
            [
                t_pt.x() + leg_length * lane_angle.value_unsafe.cos(),
                t_pt.y() + leg_length * lane_angle.value_unsafe.sin(),
            ],
        ));
        // Lower leg
        result.push((
            [t_pt.x(), t_pt.y()],
            [
                t_pt.x() - leg_length * lane_angle.value_unsafe.cos(),
                t_pt.y() - leg_length * lane_angle.value_unsafe.sin(),
            ],
        ));

        dist_along += tile_every;
    }

    result
}

fn calculate_driving_lines(road: &map_model::Road) -> Vec<(Vec2d, Vec2d)> {
    // Only multi-lane roads have dashed white lines.
    if road.offset == 0 {
        return Vec::new();
    }

    // Project left, so reverse the points.
    let mut center_pts = road.lane_center_pts.clone();
    center_pts.reverse();
    let lane_edge_pts = map_model::shift_polyline(geometry::LANE_THICKNESS / 2.0, &center_pts);

    // This is an incredibly expensive way to compute dashed polyines, and it doesn't follow bends
    // properly. Just a placeholder.
    let lane_len = geometry::polyline_len(&lane_edge_pts);
    let dash_separation = 2.0 * si::M;
    let dash_len = 1.0 * si::M;

    let mut dashes: Vec<(Vec2d, Vec2d)> = Vec::new();
    let mut start = dash_separation;
    loop {
        if start + dash_len >= lane_len - dash_separation {
            break;
        }

        let (pt1, _) = geometry::dist_along(&lane_edge_pts, start);
        let (pt2, _) = geometry::dist_along(&lane_edge_pts, start + dash_len);
        dashes.push((pt1.to_vec(), pt2.to_vec()));
        start += dash_len + dash_separation;
    }
    dashes
}

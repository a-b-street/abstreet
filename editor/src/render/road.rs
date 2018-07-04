// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::{ColorScheme, Colors};
use dimensioned::si;
use ezgui::GfxCtx;
use geom::Line;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model;
use map_model::{geometry, RoadID};
use render::PARCEL_BOUNDARY_THICKNESS;

#[derive(Debug)]
struct Marking {
    lines: Vec<[f64; 4]>,
    color: Colors,
    thickness: f64,
    round: bool,
}

#[derive(Debug)]
pub struct DrawRoad {
    pub id: RoadID,
    pub polygons: Vec<Vec<Vec2d>>,
    start_crossing: (Vec2d, Vec2d),
    end_crossing: (Vec2d, Vec2d),
    markings: Vec<Marking>,
}

impl DrawRoad {
    pub fn new(road: &map_model::Road, map: &map_model::Map) -> DrawRoad {
        let start = perp_line(road.first_line(), geometry::LANE_THICKNESS);
        let end = perp_line(road.last_line().reverse(), geometry::LANE_THICKNESS);

        let polygons = road.lane_center_pts
            .make_polygons_blindly(geometry::LANE_THICKNESS);

        let mut markings: Vec<Marking> = Vec::new();
        if road.use_yellow_center_lines {
            markings.push(Marking {
                lines: road.unshifted_pts
                    .points()
                    .windows(2)
                    .map(|pair| [pair[0].x(), pair[0].y(), pair[1].x(), pair[1].y()])
                    .collect(),
                color: Colors::RoadOrientation,
                thickness: geometry::BIG_ARROW_THICKNESS,
                round: true,
            });
        }
        for m in match road.lane_type {
            map_model::LaneType::Sidewalk => Some(calculate_sidewalk_lines(road)),
            map_model::LaneType::Parking => Some(calculate_parking_lines(road)),
            map_model::LaneType::Driving => calculate_driving_lines(road),
        } {
            markings.push(m);
        }
        // TODO not all sides of the road have to stop
        if road.lane_type == map_model::LaneType::Driving
            && !map.get_i(road.dst_i).has_traffic_signal
        {
            markings.push(calculate_stop_sign_line(road));
        }

        DrawRoad {
            id: road.id,
            polygons,
            markings,
            start_crossing: ([start[0], start[1]], [start[2], start[3]]),
            end_crossing: ([end[0], end[1]], [end[2], end[3]]),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        for p in &self.polygons {
            poly.draw(p, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }
    }

    pub fn draw_detail(&self, g: &mut GfxCtx, cs: &ColorScheme) {
        for m in &self.markings {
            let line = if m.round {
                graphics::Line::new_round(cs.get(m.color), m.thickness)
            } else {
                graphics::Line::new(cs.get(m.color), m.thickness)
            };
            for pts in &m.lines {
                line.draw(*pts, &g.ctx.draw_state, g.ctx.transform, g.gfx);
            }
        }
    }

    pub fn draw_debug(&self, g: &mut GfxCtx, cs: &ColorScheme, r: &map_model::Road) {
        let line =
            graphics::Line::new_round(cs.get(Colors::Debug), PARCEL_BOUNDARY_THICKNESS / 2.0);
        let circle = graphics::Ellipse::new(cs.get(Colors::BrightDebug));

        for pair in r.lane_center_pts.points().windows(2) {
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
// TODO move to Line or reimplement differently
fn perp_line(l: Line, length: f64) -> [f64; 4] {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    [pt1.x(), pt1.y(), pt2.x(), pt2.y()]
}

fn calculate_sidewalk_lines(road: &map_model::Road) -> Marking {
    let tile_every = geometry::LANE_THICKNESS * si::M;

    let length = road.length();

    let mut lines = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, angle) = road.dist_along(dist_along);
        // Reuse perp_line. Project away an arbitrary amount
        let pt2 = pt.project_away(1.0, angle);
        lines.push(perp_line(Line::new(pt, pt2), geometry::LANE_THICKNESS));
        dist_along += tile_every;
    }

    Marking {
        lines,
        color: Colors::SidewalkMarking,
        thickness: 0.25,
        round: false,
    }
}

fn calculate_parking_lines(road: &map_model::Road) -> Marking {
    // TODO look up this value
    let tile_every = 10.0 * si::M;
    // meters, but the dims get annoying below to remove
    // TODO make Pt2D natively understand meters, projecting away by an angle
    let leg_length = 1.0;

    let length = road.length();

    let mut lines = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, lane_angle) = road.dist_along(dist_along);
        let perp_angle = lane_angle.rotate_degs(270.0);
        // Find the outside of the lane. Actually, shift inside a little bit, since the line will
        // have thickness, but shouldn't really intersect the adjacent line when drawn.
        let t_pt = pt.project_away(geometry::LANE_THICKNESS * 0.4, perp_angle);
        // The perp leg
        let p1 = t_pt.project_away(leg_length, perp_angle.opposite());
        lines.push([t_pt.x(), t_pt.y(), p1.x(), p1.y()]);
        // Upper leg
        let p2 = t_pt.project_away(leg_length, lane_angle);
        lines.push([t_pt.x(), t_pt.y(), p2.x(), p2.y()]);
        // Lower leg
        let p3 = t_pt.project_away(leg_length, lane_angle.opposite());
        lines.push([t_pt.x(), t_pt.y(), p3.x(), p3.y()]);

        dist_along += tile_every;
    }

    Marking {
        lines,
        color: Colors::ParkingMarking,
        thickness: 0.25,
        round: false,
    }
}

fn calculate_driving_lines(road: &map_model::Road) -> Option<Marking> {
    // Only multi-lane roads have dashed white lines.
    if road.offset == 0 {
        return None;
    }

    // Project left, so reverse the points.
    let center_pts = road.lane_center_pts.reversed();
    let lane_edge_pts = center_pts.shift_blindly(geometry::LANE_THICKNESS / 2.0);

    // This is an incredibly expensive way to compute dashed polyines, and it doesn't follow bends
    // properly. Just a placeholder.
    let lane_len = lane_edge_pts.length();
    let dash_separation = 2.0 * si::M;
    let dash_len = 1.0 * si::M;

    let mut lines = Vec::new();
    let mut start = dash_separation;
    loop {
        if start + dash_len >= lane_len - dash_separation {
            break;
        }

        let (pt1, _) = lane_edge_pts.dist_along(start);
        let (pt2, _) = lane_edge_pts.dist_along(start + dash_len);
        lines.push([pt1.x(), pt1.y(), pt2.x(), pt2.y()]);
        start += dash_len + dash_separation;
    }

    Some(Marking {
        lines,
        color: Colors::DrivingLaneMarking,
        thickness: 0.25,
        round: false,
    })
}

fn calculate_stop_sign_line(road: &map_model::Road) -> Marking {
    let (pt1, angle) = road.dist_along(road.length() - (2.0 * geometry::LANE_THICKNESS * si::M));
    // Reuse perp_line. Project away an arbitrary amount
    let pt2 = pt1.project_away(1.0, angle);
    Marking {
        lines: vec![perp_line(Line::new(pt1, pt2), geometry::LANE_THICKNESS)],
        color: Colors::StopSignMarking,
        thickness: 0.25,
        round: true,
    }
}

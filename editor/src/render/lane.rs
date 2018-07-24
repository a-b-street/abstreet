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
use map_model::{geometry, LaneID};
use render::PARCEL_BOUNDARY_THICKNESS;

#[derive(Debug)]
struct Marking {
    lines: Vec<[f64; 4]>,
    color: Colors,
    thickness: f64,
    round: bool,
}

#[derive(Debug)]
pub struct DrawLane {
    pub id: LaneID,
    pub polygons: Vec<Vec<Vec2d>>,
    start_crossing: (Vec2d, Vec2d),
    end_crossing: (Vec2d, Vec2d),
    markings: Vec<Marking>,
}

impl DrawLane {
    pub fn new(lane: &map_model::Lane, map: &map_model::Map) -> DrawLane {
        let start = perp_line(lane.first_line(), geometry::LANE_THICKNESS);
        let end = perp_line(lane.last_line().reverse(), geometry::LANE_THICKNESS);

        let polygons = lane.lane_center_pts
            .make_polygons_blindly(geometry::LANE_THICKNESS);

        let mut markings: Vec<Marking> = Vec::new();
        if lane.use_yellow_center_lines {
            markings.push(Marking {
                lines: lane.unshifted_pts
                    .points()
                    .windows(2)
                    .map(|pair| [pair[0].x(), pair[0].y(), pair[1].x(), pair[1].y()])
                    .collect(),
                color: Colors::RoadOrientation,
                thickness: geometry::BIG_ARROW_THICKNESS,
                round: true,
            });
        }
        for m in match lane.lane_type {
            map_model::LaneType::Sidewalk => Some(calculate_sidewalk_lines(lane)),
            map_model::LaneType::Parking => Some(calculate_parking_lines(lane)),
            map_model::LaneType::Driving => calculate_driving_lines(lane),
            map_model::LaneType::Biking => None,
        } {
            markings.push(m);
        }
        // TODO not all sides of the lane have to stop
        if lane.lane_type == map_model::LaneType::Driving
            && !map.get_i(lane.dst_i).has_traffic_signal
        {
            markings.push(calculate_stop_sign_line(lane));
        }

        DrawLane {
            id: lane.id,
            polygons,
            markings,
            start_crossing: ([start[0], start[1]], [start[2], start[3]]),
            end_crossing: ([end[0], end[1]], [end[2], end[3]]),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        for p in &self.polygons {
            g.draw_polygon(color, p);
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
                g.draw_line(&line, *pts);
            }
        }
    }

    pub fn draw_debug(&self, g: &mut GfxCtx, cs: &ColorScheme, l: &map_model::Lane) {
        let line =
            graphics::Line::new_round(cs.get(Colors::Debug), PARCEL_BOUNDARY_THICKNESS / 2.0);
        let circle_color = cs.get(Colors::BrightDebug);

        for pair in l.lane_center_pts.points().windows(2) {
            let (pt1, pt2) = (pair[0], pair[1]);
            g.draw_line(&line, [pt1.x(), pt1.y(), pt2.x(), pt2.y()]);
            g.draw_ellipse(circle_color, geometry::circle(pt1.x(), pt1.y(), 0.4));
            g.draw_ellipse(circle_color, geometry::circle(pt2.x(), pt2.y(), 0.8));
        }
    }

    pub fn get_bbox_for_lane(&self) -> Rect {
        geometry::get_bbox_for_polygons(&self.polygons)
    }

    pub fn lane_contains_pt(&self, x: f64, y: f64) -> bool {
        for p in &self.polygons {
            if geometry::point_in_polygon(x, y, p) {
                return true;
            }
        }
        false
    }

    pub fn tooltip_lines(&self, map: &map_model::Map) -> Vec<String> {
        let l = map.get_l(self.id);
        let mut lines = vec![
            format!(
                "{} is {}",
                l.id,
                l.osm_tags.get("name").unwrap_or(&"???".to_string())
            ),
            format!(
                "From OSM way {}, with {} polygons, orig road idx {}",
                l.osm_way_id,
                self.polygons.len(),
                l.orig_road_idx,
            ),
            format!(
                "Lane goes from {} to {}",
                map.get_source_intersection(self.id).elevation,
                map.get_destination_intersection(self.id).elevation,
            ),
            format!("Lane is {}m long", l.length()),
        ];
        for (k, v) in &l.osm_tags {
            lines.push(format!("{} = {}", k, v));
        }
        lines
    }

    // Get the line marking the end of the lane, perpendicular to the direction of the lane
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

fn calculate_sidewalk_lines(lane: &map_model::Lane) -> Marking {
    let tile_every = geometry::LANE_THICKNESS * si::M;

    let length = lane.length();

    let mut lines = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, angle) = lane.dist_along(dist_along);
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

fn calculate_parking_lines(lane: &map_model::Lane) -> Marking {
    // meters, but the dims get annoying below to remove
    // TODO make Pt2D natively understand meters, projecting away by an angle
    let leg_length = 1.0;

    let mut lines = Vec::new();
    let num_spots = lane.number_parking_spots();
    if num_spots > 0 {
        for idx in 0..=num_spots {
            let (pt, lane_angle) =
                lane.dist_along(map_model::PARKING_SPOT_LENGTH * (1.0 + idx as f64));
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
        }
    }

    Marking {
        lines,
        color: Colors::ParkingMarking,
        thickness: 0.25,
        round: false,
    }
}

fn calculate_driving_lines(lane: &map_model::Lane) -> Option<Marking> {
    // Only multi-lane lanes have dashed white lines.
    if lane.offset == 0 {
        return None;
    }

    // Project left, so reverse the points.
    let center_pts = lane.lane_center_pts.reversed();
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

fn calculate_stop_sign_line(lane: &map_model::Lane) -> Marking {
    let (pt1, angle) = lane.dist_along(lane.length() - (2.0 * geometry::LANE_THICKNESS * si::M));
    // Reuse perp_line. Project away an arbitrary amount
    let pt2 = pt1.project_away(1.0, angle);
    Marking {
        lines: vec![perp_line(Line::new(pt1, pt2), geometry::LANE_THICKNESS)],
        color: Colors::StopSignMarking,
        thickness: 0.25,
        round: true,
    }
}

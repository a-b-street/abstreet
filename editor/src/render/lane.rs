// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::{ColorScheme, Colors};
use control::ControlMap;
use dimensioned::si;
use ezgui::{Canvas, GfxCtx};
use geom::{Line, Polygon, Pt2D};
use graphics;
use graphics::types::Color;
use map_model;
use map_model::{geometry, LaneID};
use render::{get_bbox, Renderable, PARCEL_BOUNDARY_THICKNESS};

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
    pub polygon: Polygon,
    start_crossing: Line,
    end_crossing: Line,
    markings: Vec<Marking>,

    // TODO pretty temporary
    draw_id_at: Vec<Pt2D>,
}

impl DrawLane {
    pub fn new(lane: &map_model::Lane, map: &map_model::Map, control_map: &ControlMap) -> DrawLane {
        let road = map.get_r(lane.parent);
        let start = new_perp_line(lane.first_line(), geometry::LANE_THICKNESS);
        let end = new_perp_line(lane.last_line().reverse(), geometry::LANE_THICKNESS);
        let polygon = lane.lane_center_pts
            .make_polygons_blindly(geometry::LANE_THICKNESS);

        let mut markings: Vec<Marking> = Vec::new();
        if road.is_canonical_lane(lane.id) {
            markings.push(Marking {
                lines: road.center_pts
                    .points()
                    .windows(2)
                    .map(|pair| [pair[0].x(), pair[0].y(), pair[1].x(), pair[1].y()])
                    .collect(),
                color: Colors::RoadOrientation,
                thickness: geometry::BIG_ARROW_THICKNESS,
                round: true,
            });
        }
        match lane.lane_type {
            map_model::LaneType::Sidewalk => {
                markings.push(calculate_sidewalk_lines(lane));
                for s in &lane.bus_stops {
                    markings.push(calculate_bus_stop_lines(s, lane));
                }
            }
            map_model::LaneType::Parking => {
                markings.push(calculate_parking_lines(lane));
            }
            map_model::LaneType::Driving => {
                for m in calculate_driving_lines(lane, road) {
                    markings.push(m);
                }
            }
            map_model::LaneType::Biking => {}
        };
        if lane.is_driving() && !map.get_i(lane.dst_i).has_traffic_signal {
            if let Some(m) = calculate_stop_sign_line(lane, control_map) {
                markings.push(m);
            }
        }

        DrawLane {
            id: lane.id,
            polygon,
            markings,
            start_crossing: start,
            end_crossing: end,
            draw_id_at: calculate_id_positions(lane).unwrap_or(Vec::new()),
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

    pub fn draw_debug(
        &self,
        g: &mut GfxCtx,
        canvas: &Canvas,
        cs: &ColorScheme,
        l: &map_model::Lane,
    ) {
        let line =
            graphics::Line::new_round(cs.get(Colors::Debug), PARCEL_BOUNDARY_THICKNESS / 2.0);
        let circle_color = cs.get(Colors::BrightDebug);

        for pair in l.lane_center_pts.points().windows(2) {
            let (pt1, pt2) = (pair[0], pair[1]);
            g.draw_line(&line, [pt1.x(), pt1.y(), pt2.x(), pt2.y()]);
            g.draw_ellipse(circle_color, geometry::make_circle(pt1, 0.4));
            g.draw_ellipse(circle_color, geometry::make_circle(pt2, 0.8));
        }

        for pt in &self.draw_id_at {
            canvas.draw_text_at(g, &vec![format!("{}", self.id.0)], pt.x(), pt.y());
        }
    }

    // Get the line marking the end of the lane, perpendicular to the direction of the lane
    pub fn get_end_crossing(&self) -> &Line {
        &self.end_crossing
    }

    pub fn get_start_crossing(&self) -> &Line {
        &self.start_crossing
    }
}

impl Renderable for DrawLane {
    type ID = LaneID;

    fn get_id(&self) -> LaneID {
        self.id
    }

    fn draw(&self, g: &mut GfxCtx, color: Color, _cs: &ColorScheme) {
        g.draw_polygon(color, &self.polygon);
    }

    fn get_bbox(&self) -> Rect {
        get_bbox(&self.polygon.get_bounds())
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, map: &map_model::Map) -> Vec<String> {
        let l = map.get_l(self.id);
        let r = map.get_r(l.parent);
        let mut lines = vec![
            format!(
                "{} is {}",
                l.id,
                r.osm_tags.get("name").unwrap_or(&"???".to_string())
            ),
            format!("From OSM way {}, parent is {}", r.osm_way_id, r.id,),
            format!(
                "Lane goes from {} to {}",
                map.get_source_intersection(self.id).elevation,
                map.get_destination_intersection(self.id).elevation,
            ),
            format!("Lane is {}m long", l.length()),
        ];
        for (k, v) in &r.osm_tags {
            lines.push(format!("{} = {}", k, v));
        }
        lines
    }
}

// TODO this always does it at pt1
// TODO move to Line or reimplement differently
fn perp_line(l: Line, length: f64) -> [f64; 4] {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    [pt1.x(), pt1.y(), pt2.x(), pt2.y()]
}

fn new_perp_line(l: Line, length: f64) -> Line {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    Line::new(pt1, pt2)
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

fn calculate_driving_lines(lane: &map_model::Lane, parent: &map_model::Road) -> Option<Marking> {
    // The rightmost lanes don't have dashed white lines.
    if parent.dir_and_offset(lane.id).1 == 0 {
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

fn calculate_stop_sign_line(lane: &map_model::Lane, control_map: &ControlMap) -> Option<Marking> {
    if control_map.stop_signs[&lane.dst_i].is_priority_lane(lane.id) {
        return None;
    }

    // TODO maybe draw the stop sign octagon on each lane?

    let (pt1, angle) =
        lane.safe_dist_along(lane.length() - (2.0 * geometry::LANE_THICKNESS * si::M))?;
    // Reuse perp_line. Project away an arbitrary amount
    let pt2 = pt1.project_away(1.0, angle);
    Some(Marking {
        lines: vec![perp_line(Line::new(pt1, pt2), geometry::LANE_THICKNESS)],
        color: Colors::StopSignMarking,
        thickness: 0.45,
        round: true,
    })
}

fn calculate_id_positions(lane: &map_model::Lane) -> Option<Vec<Pt2D>> {
    if !lane.is_driving() {
        return None;
    }

    let (pt1, _) = lane.safe_dist_along(lane.length() - (2.0 * geometry::LANE_THICKNESS * si::M))?;
    let (pt2, _) = lane.safe_dist_along(2.0 * geometry::LANE_THICKNESS * si::M)?;
    Some(vec![pt1, pt2])
}

fn calculate_bus_stop_lines(stop: &map_model::BusStopDetails, lane: &map_model::Lane) -> Marking {
    let radius = 2.0 * si::M;
    Marking {
        // TODO if this happens to cross a bend in the lane, it'll look weird. similar to the
        // lookahead arrows and center points / dashed white, we really want to render an Interval
        // or something.
        // Kinda sad that bus stops might be very close to the start of the lane, but it's
        // happening.
        lines: vec![geometry::drawing_line(&Line::new(
            lane.safe_dist_along(stop.dist_along - radius)
                .map(|(pt, _)| pt)
                .unwrap_or(lane.first_pt()),
            lane.safe_dist_along(stop.dist_along + radius)
                .map(|(pt, _)| pt)
                .unwrap_or(lane.last_pt()),
        ))],
        color: Colors::BusStopMarking,
        thickness: 0.8 * geometry::LANE_THICKNESS,
        round: true,
    }
}

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::ColorScheme;
use control::ControlMap;
use dimensioned::si;
use ezgui::{Color, GfxCtx, Text};
use geom::{Bounds, Circle, Line, Polygon, Pt2D};
use map_model::{Lane, LaneID, LaneType, Map, Road, LANE_THICKNESS, PARKING_SPOT_LENGTH};
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable, BIG_ARROW_THICKNESS, PARCEL_BOUNDARY_THICKNESS};
use sim::Sim;

const MIN_ZOOM_FOR_LANE_MARKERS: f64 = 5.0;

struct Marking {
    lines: Vec<Line>,
    // Weird indirection to keep the color definition close to the marking definition, without
    // needing to plumb in a ColorScheme immediately.
    color: Box<Fn(&mut ColorScheme) -> Color>,
    thickness: f64,
    round: bool,
    arrow_head_length: Option<f64>,
}

pub struct DrawLane {
    pub id: LaneID,
    pub polygon: Polygon,
    markings: Vec<Marking>,

    // TODO pretty temporary
    draw_id_at: Vec<Pt2D>,
}

impl DrawLane {
    pub fn new(lane: &Lane, map: &Map, control_map: &ControlMap) -> DrawLane {
        let road = map.get_r(lane.parent);
        let polygon = lane.lane_center_pts.make_polygons_blindly(LANE_THICKNESS);

        let mut markings: Vec<Marking> = Vec::new();
        if road.is_canonical_lane(lane.id) {
            markings.push(Marking {
                lines: road.center_pts.lines(),
                color: Box::new(|cs| cs.get("road center line", Color::YELLOW)),
                thickness: BIG_ARROW_THICKNESS,
                round: true,
                arrow_head_length: None,
            });
        }
        match lane.lane_type {
            LaneType::Sidewalk => {
                markings.push(calculate_sidewalk_lines(lane));
            }
            LaneType::Parking => {
                markings.push(calculate_parking_lines(lane));
            }
            LaneType::Driving | LaneType::Bus => {
                for m in calculate_driving_lines(lane, road) {
                    markings.push(m);
                }
                for m in calculate_turn_markings(map, lane, road) {
                    markings.push(m);
                }
            }
            LaneType::Biking => {}
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
            draw_id_at: calculate_id_positions(lane).unwrap_or(Vec::new()),
        }
    }

    fn draw_debug(&self, g: &mut GfxCtx, ctx: Ctx) {
        let circle_color = ctx
            .cs
            .get("debug line endpoint", Color::rgb_f(0.8, 0.1, 0.1));

        for l in ctx.map.get_l(self.id).lane_center_pts.lines() {
            g.draw_line(
                ctx.cs.get("debug line", Color::RED),
                PARCEL_BOUNDARY_THICKNESS / 2.0,
                &l,
            );
            g.draw_circle(circle_color, &Circle::new(l.pt1(), 0.4));
            g.draw_circle(circle_color, &Circle::new(l.pt2(), 0.8));
        }

        for pt in &self.draw_id_at {
            let mut txt = Text::new();
            txt.add_line(format!("{}", self.id.0));
            ctx.canvas.draw_text_at(g, txt, *pt);
        }
    }
}

impl Renderable for DrawLane {
    fn get_id(&self) -> ID {
        ID::Lane(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            let l = ctx.map.get_l(self.id);
            let mut default = match l.lane_type {
                LaneType::Driving => ctx.cs.get("driving lane", Color::BLACK),
                LaneType::Bus => ctx.cs.get("bus lane", Color::rgb(190, 74, 76)),
                LaneType::Parking => ctx.cs.get("parking lane", Color::grey(0.2)),
                LaneType::Sidewalk => ctx.cs.get("sidewalk", Color::grey(0.8)),
                LaneType::Biking => ctx.cs.get("bike lane", Color::rgb(15, 125, 75)),
            };
            if l.probably_broken {
                default = ctx.cs.get("broken lane", Color::rgb_f(1.0, 0.0, 0.565));
            }
            default
        });
        g.draw_polygon(color, &self.polygon);

        if opts.cam_zoom >= MIN_ZOOM_FOR_LANE_MARKERS {
            for m in &self.markings {
                for line in &m.lines {
                    if let Some(head_length) = m.arrow_head_length {
                        if m.round {
                            g.draw_rounded_arrow((m.color)(ctx.cs), m.thickness, head_length, line);
                        } else {
                            g.draw_arrow((m.color)(ctx.cs), m.thickness, head_length, line);
                        }
                    } else if m.round {
                        g.draw_rounded_line((m.color)(ctx.cs), m.thickness, line);
                    } else {
                        g.draw_line((m.color)(ctx.cs), m.thickness, line);
                    }
                }
            }
        }

        if opts.debug_mode {
            self.draw_debug(g, ctx);
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, map: &Map, _sim: &Sim) -> Vec<String> {
        let l = map.get_l(self.id);
        let r = map.get_r(l.parent);
        let i1 = map.get_source_intersection(self.id);
        let i2 = map.get_destination_intersection(self.id);

        let mut lines = vec![
            format!(
                "{} is {}",
                l.id,
                r.osm_tags.get("name").unwrap_or(&"???".to_string())
            ),
            format!("From OSM way {}", r.osm_way_id),
            format!("Parent {} points to {}", r.id, r.dst_i),
            format!("Lane goes from {} to {}", i1.elevation, i2.elevation),
            format!("Lane is {} long", l.length()),
        ];
        for (k, v) in &r.osm_tags {
            lines.push(format!("{} = {}", k, v));
        }
        lines
    }
}

// TODO this always does it at pt1
fn perp_line(l: Line, length: f64) -> Line {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    Line::new(pt1, pt2)
}

fn calculate_sidewalk_lines(lane: &Lane) -> Marking {
    let tile_every = LANE_THICKNESS * si::M;

    let length = lane.length();

    let mut lines = Vec::new();
    // Start away from the intersections
    let mut dist_along = tile_every;
    while dist_along < length - tile_every {
        let (pt, angle) = lane.dist_along(dist_along);
        // Reuse perp_line. Project away an arbitrary amount
        let pt2 = pt.project_away(1.0, angle);
        lines.push(perp_line(Line::new(pt, pt2), LANE_THICKNESS));
        dist_along += tile_every;
    }

    Marking {
        lines,
        color: Box::new(|cs| cs.get("sidewalk lines", Color::grey(0.7))),
        thickness: 0.25,
        round: false,
        arrow_head_length: None,
    }
}

fn calculate_parking_lines(lane: &Lane) -> Marking {
    // meters, but the dims get annoying below to remove
    // TODO make Pt2D natively understand meters, projecting away by an angle
    let leg_length = 1.0;

    let mut lines = Vec::new();
    let num_spots = lane.number_parking_spots();
    if num_spots > 0 {
        for idx in 0..=num_spots {
            let (pt, lane_angle) = lane.dist_along(PARKING_SPOT_LENGTH * (1.0 + idx as f64));
            let perp_angle = lane_angle.rotate_degs(270.0);
            // Find the outside of the lane. Actually, shift inside a little bit, since the line will
            // have thickness, but shouldn't really intersect the adjacent line when drawn.
            let t_pt = pt.project_away(LANE_THICKNESS * 0.4, perp_angle);
            // The perp leg
            let p1 = t_pt.project_away(leg_length, perp_angle.opposite());
            lines.push(Line::new(t_pt, p1));
            // Upper leg
            let p2 = t_pt.project_away(leg_length, lane_angle);
            lines.push(Line::new(t_pt, p2));
            // Lower leg
            let p3 = t_pt.project_away(leg_length, lane_angle.opposite());
            lines.push(Line::new(t_pt, p3));
        }
    }

    Marking {
        lines,
        color: Box::new(|cs| cs.get("parking line", Color::WHITE)),
        thickness: 0.25,
        round: false,
        arrow_head_length: None,
    }
}

fn calculate_driving_lines(lane: &Lane, parent: &Road) -> Option<Marking> {
    // The leftmost lanes don't have dashed white lines.
    if parent.dir_and_offset(lane.id).1 == 0 {
        return None;
    }

    // Project left, so reverse the points.
    let center_pts = lane.lane_center_pts.reversed();
    let lane_edge_pts = center_pts.shift_blindly(LANE_THICKNESS / 2.0);

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
        lines.push(Line::new(pt1, pt2));
        start += dash_len + dash_separation;
    }

    Some(Marking {
        lines,
        color: Box::new(|cs| cs.get("dashed lane line", Color::WHITE)),
        thickness: 0.25,
        round: false,
        arrow_head_length: None,
    })
}

fn calculate_stop_sign_line(lane: &Lane, control_map: &ControlMap) -> Option<Marking> {
    if control_map.stop_signs[&lane.dst_i].is_priority_lane(lane.id) {
        return None;
    }

    // TODO maybe draw the stop sign octagon on each lane?

    let (pt1, angle) = lane.safe_dist_along(lane.length() - (2.0 * LANE_THICKNESS * si::M))?;
    // Reuse perp_line. Project away an arbitrary amount
    let pt2 = pt1.project_away(1.0, angle);
    Some(Marking {
        lines: vec![perp_line(Line::new(pt1, pt2), LANE_THICKNESS)],
        color: Box::new(|cs| cs.get("stop line for lane", Color::RED)),
        thickness: 0.45,
        round: true,
        arrow_head_length: None,
    })
}

fn calculate_id_positions(lane: &Lane) -> Option<Vec<Pt2D>> {
    if !lane.is_driving() {
        return None;
    }

    let (pt1, _) = lane.safe_dist_along(lane.length() - (2.0 * LANE_THICKNESS * si::M))?;
    let (pt2, _) = lane.safe_dist_along(2.0 * LANE_THICKNESS * si::M)?;
    Some(vec![pt1, pt2])
}

fn calculate_turn_markings(map: &Map, lane: &Lane, road: &Road) -> Vec<Marking> {
    let mut results: Vec<Marking> = Vec::new();

    // Are there multiple driving lanes on this side of the road?
    if road
        .get_siblings(lane.id)
        .into_iter()
        .find(|(_, lt)| *lt == LaneType::Driving)
        .is_none()
    {
        return results;
    }

    // If the lane's too small, don't bother.
    // TODO Maybe a Trace for the common line would actually look fine.
    if let Some((base_pt, base_angle)) = lane.safe_dist_along(lane.length() - 5.0 * si::M) {
        // Common line base
        results.push(Marking {
            lines: vec![Line::new(
                base_pt,
                base_pt.project_away(2.0, base_angle.opposite()),
            )],
            color: Box::new(|cs| cs.get("turn restrictions on lane", Color::WHITE).alpha(0.8)),
            thickness: 0.1,
            round: true,
            arrow_head_length: None,
        });

        for turn in map.get_turns_from_lane(lane.id) {
            results.push(Marking {
                lines: vec![Line::new(
                    base_pt,
                    base_pt.project_away(LANE_THICKNESS / 2.0, turn.line.angle()),
                )],
                color: Box::new(|cs| cs.get("turn restrictions on lane", Color::WHITE).alpha(0.8)),
                thickness: 0.1,
                round: true,
                arrow_head_length: Some(0.5),
            });
        }
    }

    results
}

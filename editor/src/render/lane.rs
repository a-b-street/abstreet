use crate::colors::ColorScheme;
use crate::objects::{Ctx, ID};
use crate::render::{RenderOptions, Renderable, BIG_ARROW_THICKNESS, PARCEL_BOUNDARY_THICKNESS};
use dimensioned::si;
use ezgui::{Color, GfxCtx, Text};
use geom::{Bounds, Circle, Line, Polygon, Pt2D};
use map_model::{
    IntersectionType, Lane, LaneID, LaneType, Map, Road, Turn, LANE_THICKNESS, PARKING_SPOT_LENGTH,
};

const MIN_ZOOM_FOR_LANE_MARKERS: f64 = 5.0;

// Just a function to draw something later.
// TODO It's not ideal to delay the call to ColorScheme, but it's also weird to plumb it through
// DrawMap creation.
type Marking = Box<Fn(&mut GfxCtx, &mut ColorScheme)>;

pub struct DrawLane {
    pub id: LaneID,
    pub polygon: Polygon,
    markings: Vec<Marking>,

    // TODO pretty temporary
    draw_id_at: Vec<Pt2D>,
}

impl DrawLane {
    pub fn new(lane: &Lane, map: &Map) -> DrawLane {
        let road = map.get_r(lane.parent);
        let polygon = lane.lane_center_pts.make_polygons_blindly(LANE_THICKNESS);

        let mut markings: Vec<Marking> = Vec::new();
        if road.is_canonical_lane(lane.id) {
            let lines = road.center_pts.lines();
            markings.push(Box::new(move |g, cs| {
                for line in &lines {
                    g.draw_rounded_line(
                        cs.get("road center line", Color::YELLOW),
                        BIG_ARROW_THICKNESS,
                        line,
                    );
                }
            }));
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
                for m in calculate_turn_markings(map, lane) {
                    markings.push(m);
                }
            }
            LaneType::Biking => {}
        };
        if lane.is_driving()
            && map.get_i(lane.dst_i).intersection_type == IntersectionType::StopSign
        {
            if let Some(m) = calculate_stop_sign_line(lane, map) {
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
                m(g, ctx.cs);
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

    Box::new(move |g, cs| {
        for line in &lines {
            g.draw_line(cs.get("sidewalk lines", Color::grey(0.7)), 0.25, line);
        }
    })
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

    Box::new(move |g, cs| {
        for line in &lines {
            g.draw_line(cs.get("parking line", Color::WHITE), 0.25, line);
        }
    })
}

fn calculate_driving_lines(lane: &Lane, parent: &Road) -> Option<Marking> {
    // The leftmost lanes don't have dashed white lines.
    if parent.dir_and_offset(lane.id).1 == 0 {
        return None;
    }

    let dash_separation = 1.5 * si::M;
    let dash_len = 1.0 * si::M;

    // Project left, so reverse the points.
    let lane_edge_pts = lane
        .lane_center_pts
        .reversed()
        .shift_blindly(LANE_THICKNESS / 2.0);
    if lane_edge_pts.length() < 2.0 * dash_separation {
        return None;
    }
    // Don't draw the dashes too close to the ends.
    let polygons = lane_edge_pts
        .slice(dash_separation, lane_edge_pts.length() - dash_separation)
        .0
        .dashed_polygons(0.25, dash_len, dash_separation);

    Some(Box::new(move |g, cs| {
        for p in &polygons {
            g.draw_polygon(cs.get("dashed lane line", Color::WHITE), p);
        }
    }))
}

fn calculate_stop_sign_line(lane: &Lane, map: &Map) -> Option<Marking> {
    if map.get_stop_sign(lane.dst_i).is_priority_lane(lane.id) {
        return None;
    }

    // TODO maybe draw the stop sign octagon on each lane?

    let (pt1, angle) = lane.safe_dist_along(lane.length() - (2.0 * LANE_THICKNESS * si::M))?;
    // Reuse perp_line. Project away an arbitrary amount
    let pt2 = pt1.project_away(1.0, angle);
    let line = perp_line(Line::new(pt1, pt2), LANE_THICKNESS);

    Some(Box::new(move |g, cs| {
        g.draw_rounded_line(cs.get("stop line for lane", Color::RED), 0.45, &line);
    }))
}

fn calculate_id_positions(lane: &Lane) -> Option<Vec<Pt2D>> {
    if !lane.is_driving() {
        return None;
    }

    let (pt1, _) = lane.safe_dist_along(lane.length() - (2.0 * LANE_THICKNESS * si::M))?;
    let (pt2, _) = lane.safe_dist_along(2.0 * LANE_THICKNESS * si::M)?;
    Some(vec![pt1, pt2])
}

fn calculate_turn_markings(map: &Map, lane: &Lane) -> Vec<Marking> {
    let mut results: Vec<Marking> = Vec::new();

    // Are there multiple driving lanes on this side of the road?
    if map
        .find_closest_lane(lane.id, vec![LaneType::Driving])
        .is_err()
    {
        return results;
    }

    for turn in map.get_turns_from_lane(lane.id) {
        for m in turn_markings(turn, map) {
            results.push(m);
        }
    }
    results
}

fn turn_markings(turn: &Turn, map: &Map) -> Option<Marking> {
    let lane = map.get_l(turn.id.src);
    let len = lane.length();
    if len < 7.0 * si::M {
        return None;
    }

    let common_base = lane
        .lane_center_pts
        .slice(len - 7.0 * si::M, len - 5.0 * si::M)
        .0;
    let base_polygon = common_base.make_polygons_blindly(0.1);
    let turn_line = Line::new(
        common_base.last_pt(),
        common_base
            .last_pt()
            .project_away(LANE_THICKNESS / 2.0, turn.angle()),
    );

    Some(Box::new(move |g, cs| {
        let color = cs.get("turn restrictions on lane", Color::WHITE).alpha(0.8);
        g.draw_polygon(color, &base_polygon);
        g.draw_rounded_arrow(color, 0.05, 0.5, &turn_line);
    }))
}

use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::render::{DrawCrosswalk, DrawMap, DrawTurn, RenderOptions, Renderable};
use ezgui::{Color, GfxCtx};
use geom::{Angle, Bounds, Circle, Polygon, Pt2D};
use map_model::{
    ControlStopSign, Cycle, Intersection, IntersectionID, IntersectionType, Map, TurnID,
    TurnPriority, TurnType, LANE_THICKNESS,
};
use std::collections::HashSet;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub polygon: Polygon,
    pub crosswalks: Vec<DrawCrosswalk>,
    sidewalk_corners: Vec<Polygon>,
    center: Pt2D,
    intersection_type: IntersectionType,
    should_draw_stop_sign: bool,
}

impl DrawIntersection {
    pub fn new(inter: &Intersection, map: &Map) -> DrawIntersection {
        // Don't skew the center towards the repeated point
        let mut pts = inter.polygon.clone();
        pts.pop();
        let center = Pt2D::center(&pts);

        DrawIntersection {
            center,
            id: inter.id,
            polygon: Polygon::new(&inter.polygon),
            crosswalks: calculate_crosswalks(inter.id, map),
            sidewalk_corners: calculate_corners(inter.id, map),
            intersection_type: inter.intersection_type,
            should_draw_stop_sign: inter.intersection_type == IntersectionType::StopSign
                && !inter.is_degenerate(),
        }
    }

    fn draw_stop_sign(&self, g: &mut GfxCtx, ctx: &Ctx) {
        g.draw_polygon(
            ctx.cs.get_def("stop sign background", Color::RED),
            &Polygon::regular_polygon(self.center, 8, 1.5, Angle::new_degs(360.0 / 16.0)),
        );
        // TODO draw "STOP"
    }

    fn draw_traffic_signal(&self, g: &mut GfxCtx, ctx: &Ctx) {
        let radius = 0.5;

        g.draw_polygon(
            ctx.cs.get_def("traffic signal box", Color::BLACK),
            &Polygon::rectangle(self.center, 4.0 * radius, 8.0 * radius),
        );

        g.draw_circle(
            ctx.cs.get_def("traffic signal yellow", Color::YELLOW),
            &Circle::new(self.center, radius),
        );

        g.draw_circle(
            ctx.cs.get_def("traffic signal green", Color::GREEN),
            &Circle::new(self.center.offset(0.0, radius * 2.0), radius),
        );

        g.draw_circle(
            ctx.cs.get_def("traffic signal red", Color::RED),
            &Circle::new(self.center.offset(0.0, radius * -2.0), radius),
        );
    }
}

impl Renderable for DrawIntersection {
    fn get_id(&self) -> ID {
        ID::Intersection(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: &Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            if self.intersection_type == IntersectionType::Border {
                return ctx
                    .cs
                    .get_def("border intersection", Color::rgb(50, 205, 50));
            }

            let _changed = if let Some(s) = ctx.map.maybe_get_traffic_signal(self.id) {
                s.is_changed()
            } else if let Some(s) = ctx.map.maybe_get_stop_sign(self.id) {
                s.is_changed()
            } else {
                false
            };
            // TODO Make some other way to view map edits. rgb_f(0.8, 0.6, 0.6) was distracting.
            ctx.cs.get_def("unchanged intersection", Color::grey(0.6))
        });
        g.draw_polygon(color, &self.polygon);

        for crosswalk in &self.crosswalks {
            if !ctx.hints.hide_crosswalks.contains(&crosswalk.id1) {
                crosswalk.draw(
                    g,
                    *ctx.hints
                        .color_crosswalks
                        .get(&crosswalk.id1)
                        .unwrap_or(&ctx.cs.get_def("crosswalk", Color::WHITE)),
                );
            }
        }

        for corner in &self.sidewalk_corners {
            g.draw_polygon(ctx.cs.get_def("sidewalk corner", Color::grey(0.7)), corner);
        }

        if ctx.hints.suppress_intersection_icon != Some(self.id) {
            if self.intersection_type == IntersectionType::TrafficSignal {
                self.draw_traffic_signal(g, ctx);
            } else if self.should_draw_stop_sign {
                self.draw_stop_sign(g, ctx);
            }
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }
}

fn calculate_crosswalks(i: IntersectionID, map: &Map) -> Vec<DrawCrosswalk> {
    let mut crosswalks = Vec::new();
    for turn in &map.get_turns_in_intersection(i) {
        // Avoid double-rendering
        if turn.turn_type == TurnType::Crosswalk && map.get_l(turn.id.src).dst_i == i {
            crosswalks.push(DrawCrosswalk::new(turn));
        }
    }
    crosswalks
}

fn calculate_corners(i: IntersectionID, map: &Map) -> Vec<Polygon> {
    let mut corners = Vec::new();

    for turn in &map.get_turns_in_intersection(i) {
        if turn.turn_type == TurnType::SharedSidewalkCorner {
            // Avoid double-rendering
            if map.get_l(turn.id.src).dst_i != i {
                continue;
            }

            let l1 = map.get_l(turn.id.src);
            let l2 = map.get_l(turn.id.dst);

            let shared_pt1 = l1.last_line().shift(LANE_THICKNESS / 2.0).pt2();
            let pt1 = l1.last_line().reverse().shift(LANE_THICKNESS / 2.0).pt1();
            let pt2 = l2.first_line().reverse().shift(LANE_THICKNESS / 2.0).pt2();
            let shared_pt2 = l2.first_line().shift(LANE_THICKNESS / 2.0).pt1();

            corners.push(Polygon::new(&vec![shared_pt1, pt1, pt2, shared_pt2]));
        }
    }

    corners
}

pub fn draw_signal_cycle(
    cycle: &Cycle,
    g: &mut GfxCtx,
    cs: &ColorScheme,
    map: &Map,
    draw_map: &DrawMap,
    hide_crosswalks: &HashSet<TurnID>,
) {
    let priority_color = cs.get_def("turns protected by traffic signal right now", Color::GREEN);
    let yield_color = cs.get_def(
        "turns allowed with yielding by traffic signal right now",
        Color::rgba(255, 105, 180, 0.8),
    );

    for crosswalk in &draw_map.get_i(cycle.parent).crosswalks {
        if !hide_crosswalks.contains(&crosswalk.id1) {
            crosswalk.draw(g, cs.get("crosswalk"));
        }
    }
    for t in &cycle.priority_turns {
        let turn = map.get_t(*t);
        if !turn.between_sidewalks() {
            DrawTurn::draw_full(turn, g, priority_color);
        }
    }
    for t in &cycle.yield_turns {
        let turn = map.get_t(*t);
        if !turn.between_sidewalks() {
            DrawTurn::draw_dashed(turn, g, yield_color);
        }
    }
}

pub fn draw_stop_sign(sign: &ControlStopSign, g: &mut GfxCtx, cs: &ColorScheme, map: &Map) {
    let priority_color = cs.get_def("stop sign priority turns", Color::GREEN);
    // TODO pink yield color from traffic signals is nice, but it's too close to red for stop...
    let yield_color = cs.get_def("stop sign yield turns", Color::YELLOW.alpha(0.8));
    let stop_color = cs.get_def("stop sign stop turns", Color::RED.alpha(0.8));

    // TODO first crosswalks... actually, give rendering hints to override the color. dont do that
    // here.

    // First draw solid-line priority turns.
    for (t, priority) in &sign.turns {
        let turn = map.get_t(*t);
        if turn.between_sidewalks() || *priority != TurnPriority::Priority {
            continue;
        }
        DrawTurn::draw_full(turn, g, priority_color);
    }

    // Then dashed lines.
    for (t, priority) in &sign.turns {
        let turn = map.get_t(*t);
        if turn.between_sidewalks() {
            continue;
        }
        match *priority {
            TurnPriority::Yield => {
                DrawTurn::draw_dashed(turn, g, yield_color);
            }
            TurnPriority::Stop => {
                DrawTurn::draw_dashed(turn, g, stop_color);
            }
            _ => {}
        };
    }
}

pub fn stop_sign_rendering_hints(
    hints: &mut RenderingHints,
    sign: &ControlStopSign,
    map: &Map,
    cs: &ColorScheme,
) {
    hints.suppress_intersection_icon = Some(sign.id);
    for (t, pri) in &sign.turns {
        if map.get_t(*t).turn_type != TurnType::Crosswalk {
            continue;
        }
        match pri {
            // Leave the default white.
            TurnPriority::Priority => {}
            TurnPriority::Yield => {
                hints
                    .color_crosswalks
                    .insert(*t, cs.get_def("stop sign yield crosswalk", Color::YELLOW));
            }
            TurnPriority::Stop => {
                hints
                    .color_crosswalks
                    .insert(*t, cs.get_def("stop sign stop crosswalk", Color::RED));
            }
            TurnPriority::Banned => {
                hints.hide_crosswalks.insert(*t);
            }
        };
    }
}

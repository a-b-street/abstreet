// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Angle, Bounds, Circle, Line, Polygon, Pt2D};
use map_model::{Intersection, IntersectionID, Map, TurnType, LANE_THICKNESS};
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable};
use sim::Sim;
use std::f64;

const CROSSWALK_LINE_THICKNESS: f64 = 0.25;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub polygon: Polygon,
    crosswalks: Vec<Vec<Line>>,
    sidewalk_corners: Vec<Polygon>,
    center: Pt2D,
    has_traffic_signal: bool,
    is_border: bool,
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
            has_traffic_signal: inter.has_traffic_signal,
            is_border: inter.is_border(map),
            should_draw_stop_sign: !inter.has_traffic_signal && !inter.is_degenerate(),
        }
    }

    fn draw_stop_sign(&self, g: &mut GfxCtx, ctx: Ctx) {
        g.draw_polygon(
            ctx.cs.get("stop sign background", Color::RED),
            &Polygon::regular_polygon(self.center, 8, 1.5, Angle::new_degs(360.0 / 16.0)),
        );
        // TODO draw "STOP"
    }

    fn draw_traffic_signal(&self, g: &mut GfxCtx, ctx: Ctx) {
        let radius = 0.5;

        g.draw_rectangle(
            ctx.cs.get("traffic signal box", Color::BLACK),
            [
                self.center.x() - (2.0 * radius),
                self.center.y() - (4.0 * radius),
                4.0 * radius,
                8.0 * radius,
            ],
        );

        g.draw_circle(
            ctx.cs.get("traffic signal yellow", Color::YELLOW),
            &Circle::new(self.center, radius),
        );

        g.draw_circle(
            ctx.cs.get("traffic signal green", Color::GREEN),
            &Circle::new(self.center.offset(0.0, radius * 2.0), radius),
        );

        g.draw_circle(
            ctx.cs.get("traffic signal red", Color::RED),
            &Circle::new(self.center.offset(0.0, radius * -2.0), radius),
        );
    }
}

impl Renderable for DrawIntersection {
    fn get_id(&self) -> ID {
        ID::Intersection(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        let color = opts.color.unwrap_or_else(|| {
            if self.is_border {
                return ctx.cs.get("border intersection", Color::rgb(50, 205, 50));
            }

            let changed = if let Some(s) = ctx.control_map.traffic_signals.get(&self.id) {
                s.is_changed()
            } else if let Some(s) = ctx.control_map.stop_signs.get(&self.id) {
                s.is_changed()
            } else {
                false
            };
            if changed {
                ctx.cs
                    .get("changed intersection", Color::rgb_f(0.8, 0.6, 0.6))
            } else {
                ctx.cs.get("unchanged intersection", Color::grey(0.6))
            }
        });
        g.draw_polygon(color, &self.polygon);

        for crosswalk in &self.crosswalks {
            for line in crosswalk {
                g.draw_line(
                    ctx.cs.get("crosswalk", Color::WHITE),
                    CROSSWALK_LINE_THICKNESS,
                    line,
                );
            }
        }

        for corner in &self.sidewalk_corners {
            g.draw_polygon(ctx.cs.get("sidewalk corner", Color::grey(0.7)), corner);
        }

        if self.has_traffic_signal {
            self.draw_traffic_signal(g, ctx);
        } else if self.should_draw_stop_sign {
            self.draw_stop_sign(g, ctx);
        }
    }

    fn get_bounds(&self) -> Bounds {
        self.polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, map: &Map, _sim: &Sim) -> Vec<String> {
        vec![
            self.id.to_string(),
            format!("Roads: {:?}", map.get_i(self.id).roads),
        ]
    }
}

fn calculate_crosswalks(i: IntersectionID, map: &Map) -> Vec<Vec<Line>> {
    let mut crosswalks = Vec::new();

    for turn in &map.get_turns_in_intersection(i) {
        match turn.turn_type {
            TurnType::Crosswalk => {
                // Avoid double-rendering
                if map.get_l(turn.id.src).dst_i != i {
                    continue;
                }

                let mut markings = Vec::new();
                // Start at least LANE_THICKNESS out to not hit sidewalk corners. Also account for
                // the thickness of the crosswalk line itself. Center the lines inside these two
                // boundaries.
                let boundary = (LANE_THICKNESS + CROSSWALK_LINE_THICKNESS) * si::M;
                let tile_every = 0.6 * LANE_THICKNESS * si::M;
                let available_length = turn.line.length() - (2.0 * boundary);
                if available_length > 0.0 * si::M {
                    let num_markings = (available_length / tile_every).floor() as usize;
                    // Shift away so the markings stay fully inside the intersection. Lane center points don't
                    // line up with the boundary.
                    let line = turn.line.shift(LANE_THICKNESS / 2.0);

                    let mut dist_along =
                        boundary + (available_length - tile_every * (num_markings as f64)) / 2.0;
                    // TODO Seems to be an off-by-one sometimes. Not enough of these.
                    for _ in 0..=num_markings {
                        let pt1 = line.dist_along(dist_along);
                        // Reuse perp_line. Project away an arbitrary amount
                        let pt2 = pt1.project_away(1.0, turn.line.angle());
                        markings.push(perp_line(Line::new(pt1, pt2), LANE_THICKNESS));
                        dist_along += tile_every;
                    }
                    crosswalks.push(markings);
                }
            }
            // TODO render shared corners
            _ => {}
        }
    }

    crosswalks
}

fn calculate_corners(i: IntersectionID, map: &Map) -> Vec<Polygon> {
    let mut corners = Vec::new();

    for turn in &map.get_turns_in_intersection(i) {
        match turn.turn_type {
            TurnType::SharedSidewalkCorner => {
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
            _ => {}
        }
    }

    corners
}

// TODO copied from DrawLane
fn perp_line(l: Line, length: f64) -> Line {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    Line::new(pt1, pt2)
}

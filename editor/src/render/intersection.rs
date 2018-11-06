// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Angle, Bounds, Circle, Line, Polygon, Pt2D};
use map_model::{Intersection, IntersectionID, Map, TurnType, LANE_THICKNESS};
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable};
use std::f64;

const CROSSWALK_LINE_THICKNESS: f64 = 0.25;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub polygon: Polygon,
    crosswalks: Vec<Vec<Line>>,
    center: Pt2D,
    has_traffic_signal: bool,
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
            has_traffic_signal: inter.has_traffic_signal,
            should_draw_stop_sign: !inter.has_traffic_signal && !inter.is_degenerate(map),
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

    fn tooltip_lines(&self, _map: &Map) -> Vec<String> {
        vec![self.id.to_string()]
    }
}

fn calculate_crosswalks(i: IntersectionID, map: &Map) -> Vec<Vec<Line>> {
    let mut crosswalks = Vec::new();

    for turn in &map.get_turns_in_intersection(i) {
        match turn.turn_type {
            TurnType::Crosswalk => {
                // TODO don't double-render

                let mut markings = Vec::new();
                // Start at least LANE_THICKNESS out to not hit sidewalk corners. Also account for
                // the thickness of the crosswalk line itself. Center the lines inside these two
                // boundaries.
                let boundary = (LANE_THICKNESS + CROSSWALK_LINE_THICKNESS) * si::M;
                let tile_every = 0.6 * LANE_THICKNESS * si::M;
                let available_length = turn.line.length() - (2.0 * boundary);
                if available_length > 0.0 * si::M {
                    let num_markings = (available_length / tile_every).floor() as usize;

                    let mut dist_along =
                        boundary + (available_length - tile_every * (num_markings as f64)) / 2.0;
                    // TODO Seems to be an off-by-one sometimes
                    for _ in 0..=num_markings {
                        let pt1 = turn.line.dist_along(dist_along);
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

// TODO copied from DrawLane
fn perp_line(l: Line, length: f64) -> Line {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    Line::new(pt1, pt2)
}

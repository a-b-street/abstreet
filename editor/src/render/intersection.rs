// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use dimensioned::si;
use ezgui::{Color, GfxCtx};
use geom::{Angle, Bounds, Circle, Line, Polygon, Pt2D};
use map_model::{Intersection, IntersectionID, LaneType, Map, LANE_THICKNESS};
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable};
use std::f64;

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
            crosswalks: calculate_crosswalks(inter, map),
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
                    // TODO move this somewhere
                    0.25,
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

fn calculate_crosswalks(inter: &Intersection, map: &Map) -> Vec<Vec<Line>> {
    let mut crosswalks = Vec::new();

    for id in inter
        .outgoing_lanes
        .iter()
        .chain(inter.incoming_lanes.iter())
    {
        let l1 = map.get_l(*id);
        if l1.lane_type != LaneType::Sidewalk {
            continue;
        }
        let other_side = map
            .get_r(l1.parent)
            .get_opposite_lane(l1.id, LaneType::Sidewalk);
        if other_side.is_err() {
            continue;
        }
        let l2 = map.get_l(other_side.unwrap());
        if l2.id < l1.id {
            continue;
        }

        let line = if l1.src_i == inter.id {
            Line::new(l1.first_pt(), l2.last_pt())
        } else {
            Line::new(l1.last_pt(), l2.first_pt())
        };
        let angle = line.angle();
        let length = line.length();
        // TODO awkward to express it this way

        let mut markings = Vec::new();
        let tile_every = (LANE_THICKNESS * 0.6) * si::M;
        let mut dist_along = tile_every;
        while dist_along < length - tile_every {
            let pt1 = line.dist_along(dist_along);
            // Reuse perp_line. Project away an arbitrary amount
            let pt2 = pt1.project_away(1.0, angle);
            markings.push(perp_line(Line::new(pt1, pt2), LANE_THICKNESS));
            dist_along += tile_every;
        }
        crosswalks.push(markings);
    }

    crosswalks
}

// TODO copied from DrawLane
fn perp_line(l: Line, length: f64) -> Line {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    Line::new(pt1, pt2)
}

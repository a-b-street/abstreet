// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use ezgui::{Color, GfxCtx};
use geom::{Angle, Bounds, Circle, Polygon, Pt2D};
use map_model::{Intersection, IntersectionID, IntersectionType, Map, TurnType, LANE_THICKNESS};
use objects::{Ctx, ID};
use render::{DrawCrosswalk, RenderOptions, Renderable};

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

    fn draw_stop_sign(&self, g: &mut GfxCtx, ctx: Ctx) {
        g.draw_polygon(
            ctx.cs.get("stop sign background", Color::RED),
            &Polygon::regular_polygon(self.center, 8, 1.5, Angle::new_degs(360.0 / 16.0)),
        );
        // TODO draw "STOP"
    }

    fn draw_traffic_signal(&self, g: &mut GfxCtx, ctx: Ctx) {
        let radius = 0.5;

        g.draw_polygon(
            ctx.cs.get("traffic signal box", Color::BLACK),
            &Polygon::rectangle(self.center, 4.0 * radius, 8.0 * radius),
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
            if self.intersection_type == IntersectionType::Border {
                return ctx.cs.get("border intersection", Color::rgb(50, 205, 50));
            }

            let changed = if let Some(s) = ctx.map.maybe_get_traffic_signal(self.id) {
                s.is_changed()
            } else if let Some(s) = ctx.map.maybe_get_stop_sign(self.id) {
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
            crosswalk.draw(g, ctx.cs.get("crosswalk", Color::WHITE));
        }

        for corner in &self.sidewalk_corners {
            g.draw_polygon(ctx.cs.get("sidewalk corner", Color::grey(0.7)), corner);
        }

        if self.intersection_type == IntersectionType::TrafficSignal {
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

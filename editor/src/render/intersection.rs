// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::Colors;
use dimensioned::si;
use ezgui::GfxCtx;
use geom::{Circle, Line, Polygon, Pt2D};
use map_model::{geometry, Intersection, IntersectionID, LaneType, Map};
use objects::{Ctx, ID};
use render::{get_bbox, DrawLane, RenderOptions, Renderable};
use std::f64;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub polygon: Polygon,
    crosswalks: Vec<Vec<Line>>,
    center: Pt2D,
    has_traffic_signal: bool,
}

impl DrawIntersection {
    pub fn new(inter: &Intersection, map: &Map, lanes: &Vec<DrawLane>) -> DrawIntersection {
        let mut pts: Vec<Pt2D> = Vec::new();
        for l in &inter.incoming_lanes {
            let line = lanes[l.0].get_end_crossing();
            pts.push(line.pt1());
            pts.push(line.pt2());
        }
        for l in &inter.outgoing_lanes {
            let line = lanes[l.0].get_start_crossing();
            pts.push(line.pt1());
            pts.push(line.pt2());
        }

        let center = geometry::center(&pts);
        // Sort points by angle from the center
        pts.sort_by_key(|pt| center.angle_to(*pt).normalized_degrees() as i64);
        let first_pt = pts[0].clone();
        pts.push(first_pt);

        DrawIntersection {
            center,
            id: inter.id,
            polygon: Polygon::new(&pts),
            crosswalks: calculate_crosswalks(inter, map),
            has_traffic_signal: inter.has_traffic_signal,
        }
    }

    fn draw_stop_sign(&self, g: &mut GfxCtx, ctx: Ctx) {
        // TODO rotate it
        g.draw_polygon(
            ctx.cs.get(Colors::StopSignBackground),
            &geometry::regular_polygon(self.center, 8, 1.5),
        );
        // TODO draw "STOP"
    }

    fn draw_traffic_signal(&self, g: &mut GfxCtx, ctx: Ctx) {
        let radius = 0.5;

        g.draw_rectangle(
            ctx.cs.get(Colors::TrafficSignalBox),
            [
                self.center.x() - (2.0 * radius),
                self.center.y() - (4.0 * radius),
                4.0 * radius,
                8.0 * radius,
            ],
        );

        g.draw_circle(
            ctx.cs.get(Colors::TrafficSignalYellow),
            &Circle::new(self.center, radius),
        );

        g.draw_circle(
            ctx.cs.get(Colors::TrafficSignalGreen),
            &Circle::new(self.center.offset(0.0, radius * 2.0), radius),
        );

        g.draw_circle(
            ctx.cs.get(Colors::TrafficSignalRed),
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
                s.changed()
            } else if let Some(s) = ctx.control_map.stop_signs.get(&self.id) {
                s.changed()
            } else {
                false
            };
            if changed {
                ctx.cs.get(Colors::ChangedIntersection)
            } else {
                ctx.cs.get(Colors::UnchangedIntersection)
            }
        });
        g.draw_polygon(color, &self.polygon);

        for crosswalk in &self.crosswalks {
            for line in crosswalk {
                g.draw_line(
                    ctx.cs.get(Colors::Crosswalk),
                    // TODO move this somewhere
                    0.25,
                    line,
                );
            }
        }

        if self.has_traffic_signal {
            self.draw_traffic_signal(g, ctx);
        } else {
            self.draw_stop_sign(g, ctx);
        }
    }

    fn get_bbox(&self) -> Rect {
        get_bbox(&self.polygon.get_bounds())
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
        let l2 = map.get_l(
            map.get_r(l1.parent)
                .get_opposite_lane(l1.id, LaneType::Sidewalk)
                .unwrap(),
        );
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
        let tile_every = (geometry::LANE_THICKNESS * 0.6) * si::M;
        let mut dist_along = tile_every;
        while dist_along < length - tile_every {
            let pt1 = line.dist_along(dist_along);
            // Reuse perp_line. Project away an arbitrary amount
            let pt2 = pt1.project_away(1.0, angle);
            markings.push(perp_line(Line::new(pt1, pt2), geometry::LANE_THICKNESS));
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

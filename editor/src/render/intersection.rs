// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::{ColorScheme, Colors};
use dimensioned::si;
use ezgui::GfxCtx;
use geom::{Line, Pt2D};
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model;
use map_model::geometry;
use render::DrawLane;
use std::f64;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: map_model::IntersectionID,
    pub polygon: Vec<Vec2d>,
    crosswalks: Vec<Vec<(Vec2d, Vec2d)>>,
    center: Pt2D,
    has_traffic_signal: bool,
}

impl DrawIntersection {
    pub fn new(
        inter: &map_model::Intersection,
        map: &map_model::Map,
        lanes: &Vec<DrawLane>,
    ) -> DrawIntersection {
        let mut pts: Vec<Vec2d> = Vec::new();
        for l in &inter.incoming_lanes {
            let (pt1, pt2) = lanes[l.0].get_end_crossing();
            pts.push(pt1);
            pts.push(pt2);
        }
        for l in &inter.outgoing_lanes {
            let (pt1, pt2) = lanes[l.0].get_start_crossing();
            pts.push(pt1);
            pts.push(pt2);
        }

        let center = geometry::center(&pts.iter().map(|pt| Pt2D::new(pt[0], pt[1])).collect());
        // Sort points by angle from the center
        pts.sort_by_key(|pt| {
            center
                .angle_to(Pt2D::new(pt[0], pt[1]))
                .normalized_degrees() as i64
        });
        let first_pt = pts[0].clone();
        pts.push(first_pt);

        DrawIntersection {
            center,
            id: inter.id,
            polygon: pts,
            crosswalks: calculate_crosswalks(inter, map),
            has_traffic_signal: inter.has_traffic_signal,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color, cs: &ColorScheme) {
        g.draw_polygon(color, &self.polygon);

        let crosswalk_marking = graphics::Line::new(
            cs.get(Colors::Crosswalk),
            // TODO move this somewhere
            0.25,
        );
        for crosswalk in &self.crosswalks {
            for pair in crosswalk {
                g.draw_line(
                    &crosswalk_marking,
                    [pair.0[0], pair.0[1], pair.1[0], pair.1[1]],
                );
            }
        }

        if self.has_traffic_signal {
            self.draw_traffic_signal(g, cs);
        } else {
            self.draw_stop_sign(g, cs);
        }
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.polygon)
    }

    pub fn get_bbox(&self) -> Rect {
        geometry::get_bbox_for_polygons(&[self.polygon.clone()])
    }

    fn draw_stop_sign(&self, g: &mut GfxCtx, cs: &ColorScheme) {
        // TODO rotate it
        let poly: Vec<Vec2d> = geometry::regular_polygon(self.center, 8, 1.5)
            .iter()
            .map(|pt| pt.to_vec())
            .collect();
        g.draw_polygon(cs.get(Colors::StopSignBackground), &poly);
        // TODO draw "STOP"
    }

    fn draw_traffic_signal(&self, g: &mut GfxCtx, cs: &ColorScheme) {
        let radius = 0.5;

        g.draw_rectangle(
            cs.get(Colors::TrafficSignalBox),
            [
                self.center.x() - (2.0 * radius),
                self.center.y() - (4.0 * radius),
                4.0 * radius,
                8.0 * radius,
            ],
        );

        g.draw_ellipse(
            cs.get(Colors::TrafficSignalYellow),
            geometry::circle(self.center.x(), self.center.y(), radius),
        );

        g.draw_ellipse(
            cs.get(Colors::TrafficSignalGreen),
            geometry::circle(self.center.x(), self.center.y() + (radius * 2.0), radius),
        );

        g.draw_ellipse(
            cs.get(Colors::TrafficSignalRed),
            geometry::circle(self.center.x(), self.center.y() - (radius * 2.0), radius),
        );
    }
}

fn calculate_crosswalks(
    inter: &map_model::Intersection,
    map: &map_model::Map,
) -> Vec<Vec<(Vec2d, Vec2d)>> {
    let mut crosswalks = Vec::new();

    for id in inter
        .outgoing_lanes
        .iter()
        .chain(inter.incoming_lanes.iter())
    {
        let r1 = map.get_l(*id);
        if r1.lane_type != map_model::LaneType::Sidewalk {
            continue;
        }
        if r1.other_side.unwrap().0 < r1.id.0 {
            continue;
        }
        let r2 = map.get_l(r1.other_side.unwrap());

        let line = if r1.src_i == inter.id {
            Line::new(r1.first_pt(), r2.last_pt())
        } else {
            Line::new(r1.last_pt(), r2.first_pt())
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
fn perp_line(l: Line, length: f64) -> (Vec2d, Vec2d) {
    let pt1 = l.shift(length / 2.0).pt1();
    let pt2 = l.reverse().shift(length / 2.0).pt2();
    (pt1.to_vec(), pt2.to_vec())
}

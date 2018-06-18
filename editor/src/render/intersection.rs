// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

extern crate aabb_quadtree;
extern crate map_model;

use aabb_quadtree::geom::Rect;
use ezgui::canvas::GfxCtx;
use geom::geometry;
use graphics;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model::{Bounds, IntersectionID, Map};
use render::DrawRoad;
use std::f64;

#[derive(Debug)]
pub struct DrawIntersection {
    pub id: IntersectionID,
    pub point: Vec2d,

    polygon: Vec<Vec2d>,
}

impl DrawIntersection {
    pub fn new(
        inter: &map_model::Intersection,
        map: &Map,
        roads: &Vec<DrawRoad>,
        bounds: &Bounds,
    ) -> DrawIntersection {
        let mut pts: Vec<Vec2d> = Vec::new();
        for r in &map.get_roads_to_intersection(inter.id) {
            let (pt1, pt2) = roads[r.id.0].get_end_crossing();
            pts.push(pt1);
            pts.push(pt2);
        }
        for r in &map.get_roads_from_intersection(inter.id) {
            let (pt1, pt2) = roads[r.id.0].get_start_crossing();
            pts.push(pt1);
            pts.push(pt2);
        }

        let center = geometry::gps_to_screen_space(&inter.point, bounds);
        // Sort points by angle from the center
        pts.sort_by_key(|pt| {
            let mut angle = (pt[1] - center.y()).atan2(pt[0] - center.x()).to_degrees();
            if angle < 0.0 {
                angle += 360.0;
            }
            angle as i64
        });
        let first_pt = pts[0].clone();
        pts.push(first_pt);

        DrawIntersection {
            id: inter.id,
            point: [center.x(), center.y()],
            polygon: pts,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        let poly = graphics::Polygon::new(color);
        poly.draw(&self.polygon, &g.ctx.draw_state, g.ctx.transform, g.gfx);
    }

    pub fn contains_pt(&self, x: f64, y: f64) -> bool {
        geometry::point_in_polygon(x, y, &self.polygon)
    }

    pub fn get_bbox(&self) -> Rect {
        geometry::get_bbox_for_polygons(&[self.polygon.clone()])
    }
}

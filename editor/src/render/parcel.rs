// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use geom::PolyLine;
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model;
use map_model::geometry;
use render::PARCEL_BOUNDARY_THICKNESS;

#[derive(Debug)]
pub struct DrawParcel {
    pub id: map_model::ParcelID,
    // TODO should just have one. use graphics::Line for now.
    boundary_polygons: Vec<Vec<Vec2d>>,
    pub fill_polygon: Vec<Vec2d>,
}

impl DrawParcel {
    pub fn new(p: &map_model::Parcel) -> DrawParcel {
        DrawParcel {
            id: p.id,
            boundary_polygons: PolyLine::new(p.points.clone())
                .make_polygons_blindly(PARCEL_BOUNDARY_THICKNESS),
            fill_polygon: p.points.iter().map(|pt| [pt.x(), pt.y()]).collect(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, (boundary_color, fill_color): (Color, Color)) {
        for p in &self.boundary_polygons {
            g.draw_polygon(boundary_color, p);
        }
        g.draw_polygon(fill_color, &self.fill_polygon);
    }

    //pub fn contains_pt(&self, x: f64, y: f64) -> bool {}

    pub fn get_bbox(&self) -> Rect {
        geometry::get_bbox_for_polygons(&vec![self.fill_polygon.clone()])
    }
}

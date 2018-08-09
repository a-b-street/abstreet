// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use geom::{PolyLine, Polygon};
use graphics::math::Vec2d;
use graphics::types::Color;
use map_model;
use render::{get_bbox, PARCEL_BOUNDARY_THICKNESS};

#[derive(Debug)]
pub struct DrawParcel {
    pub id: map_model::ParcelID,
    // TODO should just have one. use graphics::Line for now.
    boundary_polygons: Vec<Vec<Vec2d>>,
    pub fill_polygon: Polygon,
}

impl DrawParcel {
    pub fn new(p: &map_model::Parcel) -> DrawParcel {
        DrawParcel {
            id: p.id,
            boundary_polygons: PolyLine::new(p.points.clone())
                .make_polygons_blindly(PARCEL_BOUNDARY_THICKNESS),
            fill_polygon: Polygon::new(&p.points),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, (boundary_color, fill_color): (Color, Color)) {
        for p in &self.boundary_polygons {
            g.draw_polygon(boundary_color, p);
        }
        for p in &self.fill_polygon.for_drawing() {
            g.draw_polygon(fill_color, p);
        }
    }

    //pub fn contains_pt(&self, x: f64, y: f64) -> bool {}

    pub fn get_bbox(&self) -> Rect {
        get_bbox(&self.fill_polygon.get_bounds())
    }
}

// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use graphics;
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
    fill_polygon: Vec<Vec2d>,
}

impl DrawParcel {
    pub fn new(p: &map_model::Parcel) -> DrawParcel {
        DrawParcel {
            id: p.id,
            boundary_polygons: map_model::polygons_for_polyline(
                &p.points,
                PARCEL_BOUNDARY_THICKNESS
            ),
            fill_polygon: p.points.iter().map(|pt| [pt.x(), pt.y()]).collect(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, (boundary_color, _fill_color): (Color, Color)) {
        let boundary_poly = graphics::Polygon::new(boundary_color);
        for p in &self.boundary_polygons {
            boundary_poly.draw(p, &g.ctx.draw_state, g.ctx.transform, g.gfx);
        }
        /*let fill_poly = graphics::Polygon::new(fill_color);
        fill_poly.draw(
            &self.fill_polygon,
            &g.ctx.draw_state,
            g.ctx.transform,
            g.gfx,
        );*/
    }

    //pub fn contains_pt(&self, x: f64, y: f64) -> bool {}

    pub fn get_bbox(&self) -> Rect {
        geometry::get_bbox_for_polygons(&vec![self.fill_polygon.clone()])
    }
}

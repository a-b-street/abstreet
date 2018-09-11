// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use aabb_quadtree::geom::Rect;
use colors::ColorScheme;
use ezgui::GfxCtx;
use geom::{PolyLine, Polygon, Pt2D};
use graphics::types::Color;
use map_model::{Map, Parcel, ParcelID};
use render::{get_bbox, Renderable, PARCEL_BOUNDARY_THICKNESS};

#[derive(Debug)]
pub struct DrawParcel {
    pub id: ParcelID,
    // TODO should just have one. use graphics::Line for now.
    boundary_polygon: Polygon,
    pub fill_polygon: Polygon,
}

impl DrawParcel {
    pub fn new(p: &Parcel) -> DrawParcel {
        DrawParcel {
            id: p.id,
            boundary_polygon: PolyLine::new(p.points.clone())
                .make_polygons_blindly(PARCEL_BOUNDARY_THICKNESS),
            fill_polygon: Polygon::new(&p.points),
        }
    }
}

impl Renderable for DrawParcel {
    type ID = ParcelID;

    fn get_id(&self) -> ParcelID {
        self.id
    }

    fn draw(&self, g: &mut GfxCtx, fill_color: Color, _cs: &ColorScheme) {
        g.draw_polygon(fill_color, &self.fill_polygon);
    }

    /*fn draw(&self, g: &mut GfxCtx, (boundary_color, fill_color): (Color, Color)) {
        g.draw_polygon(boundary_color, &self.boundary_polygon);
        g.draw_polygon(fill_color, &self.fill_polygon);
    }*/

    fn get_bbox(&self) -> Rect {
        get_bbox(&self.fill_polygon.get_bounds())
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.fill_polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, _map: &Map) -> Vec<String> {
        vec![self.id.to_string()]
    }
}

use aabb_quadtree::geom::Rect;
use ezgui::GfxCtx;
use geom::{Polygon, Pt2D};
use graphics::types::Color;
use kml::{ExtraShape, ExtraShapeID};
use render::{get_bbox, EXTRA_SHAPE_THICKNESS};
use std::collections::HashMap;

#[derive(Debug)]
pub struct DrawExtraShape {
    pub id: ExtraShapeID,
    polygon: Polygon,
    attributes: HashMap<String, String>,
}

impl DrawExtraShape {
    pub fn new(s: ExtraShape) -> DrawExtraShape {
        DrawExtraShape {
            id: s.id,
            polygon: s.pts.make_polygons(EXTRA_SHAPE_THICKNESS).unwrap(),
            attributes: s.attributes,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, color: Color) {
        g.draw_polygon(color, &self.polygon);
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.polygon.contains_pt(pt)
    }

    pub fn get_bbox(&self) -> Rect {
        get_bbox(&self.polygon.get_bounds())
    }

    pub fn tooltip_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (k, v) in &self.attributes {
            lines.push(format!("{} = {}", k, v));
        }
        lines
    }
}

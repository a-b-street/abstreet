use crate::helpers::{ColorScheme, ID};
use crate::render::{DrawCtx, DrawOptions, Renderable};
use ezgui::{Color, GeomBatch, GfxCtx};
use geom::Polygon;
use map_model::{Area, AreaID, AreaType, Map};

pub struct DrawArea {
    pub id: AreaID,
}

impl DrawArea {
    pub fn new(area: &Area, cs: &ColorScheme, batch: &mut GeomBatch) -> DrawArea {
        batch.push(
            match area.area_type {
                AreaType::Park => cs.get_def("park area", Color::rgb(200, 250, 204)),
                AreaType::Water => cs.get_def("water area", Color::rgb(170, 211, 223)),
            },
            area.polygon.clone(),
        );
        DrawArea { id: area.id }
    }
}

impl Renderable for DrawArea {
    fn get_id(&self) -> ID {
        ID::Area(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: &DrawOptions, ctx: &DrawCtx) {
        if let Some(color) = opts.color(self.get_id()) {
            g.draw_polygon(color, &ctx.map.get_a(self.id).polygon);
        }
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        // Since areas are so big, don't just draw the outline
        map.get_a(self.id).polygon.clone()
    }
}

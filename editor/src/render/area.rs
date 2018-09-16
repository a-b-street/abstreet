use aabb_quadtree::geom::Rect;
use colors::Colors;
use ezgui::GfxCtx;
use geom::{Polygon, Pt2D};
use map_model::{Area, AreaID, AreaType, Map};
use objects::{Ctx, ID};
use render::{get_bbox, RenderOptions, Renderable};

#[derive(Debug)]
pub struct DrawArea {
    pub id: AreaID,
    fill_polygon: Polygon,
    color: Colors,
}

impl DrawArea {
    pub fn new(area: &Area) -> DrawArea {
        DrawArea {
            id: area.id,
            fill_polygon: Polygon::new(&area.points),
            color: match area.area_type {
                AreaType::Park => Colors::ParkArea,
                AreaType::Swamp => Colors::SwampArea,
                AreaType::Water => Colors::WaterArea,
            },
        }
    }
}

impl Renderable for DrawArea {
    fn get_id(&self) -> ID {
        ID::Area(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, ctx: Ctx) {
        g.draw_polygon(
            opts.color.unwrap_or(ctx.cs.get(self.color)),
            &self.fill_polygon,
        );
    }

    fn get_bbox(&self) -> Rect {
        get_bbox(&self.fill_polygon.get_bounds())
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        self.fill_polygon.contains_pt(pt)
    }

    fn tooltip_lines(&self, map: &Map) -> Vec<String> {
        let a = map.get_a(self.id);
        let mut lines = vec![format!("{} (from OSM way {})", self.id, a.osm_way_id)];
        for (k, v) in &a.osm_tags {
            lines.push(format!("{} = {}", k, v));
        }
        lines
    }
}

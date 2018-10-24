use colors::ColorScheme;
use ezgui::{Color, GfxCtx};
use geom::{Bounds, Polygon, Pt2D};
use map_model::{Area, AreaID, AreaType, Map};
use objects::{Ctx, ID};
use render::{RenderOptions, Renderable};

#[derive(Debug)]
pub struct DrawArea {
    pub id: AreaID,
    fill_polygon: Polygon,
    // TODO precomputing this means live color picker changes won't work. :(
    color: Color,
}

impl DrawArea {
    pub fn new(area: &Area, cs: &mut ColorScheme) -> DrawArea {
        DrawArea {
            id: area.id,
            fill_polygon: area.get_polygon(),
            color: match area.area_type {
                AreaType::Park => cs.get("park area", Color::GREEN),
                AreaType::Swamp => cs.get("swamp area", Color::rgb_f(0.0, 1.0, 0.6)),
                AreaType::Water => cs.get("water area", Color::BLUE),
            },
        }
    }
}

impl Renderable for DrawArea {
    fn get_id(&self) -> ID {
        ID::Area(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, opts: RenderOptions, _ctx: Ctx) {
        g.draw_polygon(opts.color.unwrap_or(self.color), &self.fill_polygon);
    }

    fn get_bounds(&self) -> Bounds {
        self.fill_polygon.get_bounds()
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

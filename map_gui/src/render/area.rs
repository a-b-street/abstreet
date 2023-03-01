use geom::{Bounds, Pt2D, Tessellation};
use map_model::{Area, AreaID, AreaType, Map};
use widgetry::{Color, EventCtx, Fill, GeomBatch, GfxCtx, Line, Text};

use crate::colors::ColorScheme;
use crate::render::{DrawOptions, Renderable};
use crate::{AppLike, ID};

pub struct DrawArea {
    pub id: AreaID,
}

impl DrawArea {
    pub fn new(
        ctx: &EventCtx,
        area: &Area,
        cs: &ColorScheme,
        all_areas: &mut GeomBatch,
    ) -> DrawArea {
        all_areas.push(DrawArea::fill(area.area_type, cs), area.polygon.clone());
        if false {
            // TODO Need to auto-size better -- ensure it's completely contained in the polygon,
            // probably
            if let Some(name) = area.osm_tags.get("name") {
                all_areas.append(
                    Text::from(Line(name).fg(Color::BLACK))
                        .render_autocropped(ctx)
                        .scale(1.0)
                        .centered_on(area.polygon.polylabel())
                        .set_z_offset(-0.1),
                );
            }
        }

        DrawArea { id: area.id }
    }

    pub fn fill(area_type: AreaType, cs: &ColorScheme) -> Fill {
        match area_type {
            AreaType::Park => cs.grass.clone(),
            AreaType::Water => cs.water.clone(),
            AreaType::Island => cs.map_background.clone(),
            AreaType::StudyArea => cs.study_area.clone(),
        }
    }
}

impl Renderable for DrawArea {
    fn get_id(&self) -> ID {
        ID::Area(self.id)
    }

    fn draw(&self, _: &mut GfxCtx, _: &dyn AppLike, _: &DrawOptions) {}

    fn get_outline(&self, map: &Map) -> Tessellation {
        // Since areas are so big, don't just draw the outline
        Tessellation::from(map.get_a(self.id).polygon.clone())
    }

    fn get_bounds(&self, map: &Map) -> Bounds {
        map.get_a(self.id).polygon.get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_a(self.id).polygon.contains_pt(pt)
    }
}

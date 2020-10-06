use geom::Polygon;
use map_model::{Area, AreaID, AreaType, Map};
use widgetry::{Color, EventCtx, Fill, GeomBatch, GfxCtx, Line, Text};

use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable};

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
                        .render_to_batch(ctx.prerender)
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
            // MJK TODO: convert some of these to be a Fill on the theme rather than `into`
            AreaType::Park => cs.grass.clone(),
            AreaType::Water => cs.water.clone(),
            AreaType::PedestrianIsland => Color::grey(0.3).into(),
            AreaType::Island => cs.map_background.clone(),
        }
    }
}

impl Renderable for DrawArea {
    fn get_id(&self) -> ID {
        ID::Area(self.id)
    }

    fn draw(&self, _: &mut GfxCtx, _: &App, _: &DrawOptions) {}

    fn get_outline(&self, map: &Map) -> Polygon {
        // Since areas are so big, don't just draw the outline
        map.get_a(self.id).polygon.clone()
    }
}

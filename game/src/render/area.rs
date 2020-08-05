use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable};
use ezgui::{Color, EventCtx, GeomBatch, GfxCtx, Line, Text};
use geom::Polygon;
use map_model::{Area, AreaID, AreaType, Map};

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
        all_areas.push(DrawArea::color(area.area_type, cs), area.polygon.clone());
        if false {
            // TODO Z-order needs to be on top of everything
            // TODO Need to auto-size better -- ensure it's completely contained in the polygon,
            // probably
            if let Some(name) = area.osm_tags.get("name") {
                all_areas.append(
                    Text::from(Line(name).fg(Color::BLACK))
                        .render_to_batch(ctx.prerender)
                        .scale(1.0)
                        .centered_on(area.polygon.polylabel()),
                );
            }
        }

        DrawArea { id: area.id }
    }

    pub fn color(area_type: AreaType, cs: &ColorScheme) -> Color {
        match area_type {
            AreaType::Park => cs.grass,
            AreaType::Water => cs.water,
            AreaType::PedestrianIsland => Color::grey(0.3),
            AreaType::Island => cs.map_background,
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

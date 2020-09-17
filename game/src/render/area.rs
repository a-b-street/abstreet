use crate::app::App;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::render::{DrawOptions, Renderable};
use geom::Polygon;
use map_model::{Area, AreaID, AreaType, Map};
use widgetry::{Color, EventCtx, FancyColor, GeomBatch, GfxCtx, Line, Text, Texture};

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
        all_areas.fancy_push(DrawArea::color(area.area_type, cs), area.polygon.clone());
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

    pub fn color(area_type: AreaType, cs: &ColorScheme) -> FancyColor {
        match area_type {
            AreaType::Park => FancyColor::Texture(Texture::GRASS),
            AreaType::Water => FancyColor::Texture(Texture::STILL_WATER),
            AreaType::PedestrianIsland => FancyColor::RGBA(Color::grey(0.3)),
            AreaType::Island => FancyColor::RGBA(cs.map_background),
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

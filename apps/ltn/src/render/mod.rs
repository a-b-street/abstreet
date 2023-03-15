mod cells;
pub mod colors;

use geom::Distance;
use map_model::{AmenityType, ExtraPOIType, Map};
use widgetry::mapspace::DrawCustomUnzoomedShapes;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, RewriteColor, Text};

pub use cells::RenderCells;

pub fn render_poi_icons(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();
    let school = GeomBatch::load_svg(ctx, "system/assets/map/school.svg")
        .scale(0.2)
        .color(RewriteColor::ChangeAll(Color::WHITE));

    for b in map.all_buildings() {
        if b.amenities.iter().any(|a| {
            let at = AmenityType::categorize(&a.amenity_type);
            at == Some(AmenityType::School) || at == Some(AmenityType::University)
        }) {
            batch.append(school.clone().centered_on(b.polygon.polylabel()));
        }
    }

    let tfl =
        GeomBatch::load_svg(ctx, "system/assets/map/tfl_underground.svg").scale_to_fit_width(20.0);
    let national_rail =
        GeomBatch::load_svg(ctx, "system/assets/map/national_rail.svg").scale_to_fit_width(20.0);

    // TODO Toggle3Zoomed could be nicer; these're not terribly visible from afar
    for extra in map.all_extra_pois() {
        let (name, icon) = match extra.kind {
            ExtraPOIType::LondonUndergroundStation(ref name) => (name, &tfl),
            ExtraPOIType::NationalRailStation(ref name) => (name, &national_rail),
        };
        batch.append(icon.clone().centered_on(extra.pt));
        batch.append(
            Text::from(Line(name).fg(Color::WHITE))
                .bg(Color::hex("#0019A8"))
                .render_autocropped(ctx)
                .scale_to_fit_height(10.0)
                .centered_on(extra.pt.offset(0.0, icon.get_bounds().height())),
        );
    }

    ctx.upload(batch)
}

pub fn render_bus_routes(ctx: &EventCtx, map: &Map) -> Drawable {
    let mut batch = GeomBatch::new();
    for r in map.all_roads() {
        if map.get_bus_routes_on_road(r.id).is_empty() {
            continue;
        }
        // Draw dashed outlines surrounding the road
        let width = r.get_width();
        for pl in [
            r.center_pts.shift_left(width * 0.7),
            r.center_pts.shift_right(width * 0.7),
        ]
        .into_iter()
        .flatten()
        {
            batch.extend(
                *colors::BUS_ROUTE,
                pl.exact_dashed_polygons(
                    Distance::meters(2.0),
                    Distance::meters(5.0),
                    Distance::meters(2.0),
                ),
            );
        }
    }
    ctx.upload(batch)
}

/// Depending on the canvas zoom level, draws one of 2 things.
// TODO Rethink filter styles and do something better than this.
pub struct Toggle3Zoomed {
    draw_zoomed: Drawable,
    unzoomed: DrawCustomUnzoomedShapes,
}

impl Toggle3Zoomed {
    pub fn new(draw_zoomed: Drawable, unzoomed: DrawCustomUnzoomedShapes) -> Self {
        Self {
            draw_zoomed,
            unzoomed,
        }
    }

    pub fn empty(ctx: &EventCtx) -> Self {
        Self::new(Drawable::empty(ctx), DrawCustomUnzoomedShapes::empty())
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if !self.unzoomed.maybe_draw(g) {
            self.draw_zoomed.draw(g);
        }
    }
}

use geom::Distance;
use map_model::Map;
use widgetry::mapspace::{DummyID, World};
use widgetry::{Color, EventCtx, GeomBatch, GfxCtx, Text};

pub struct CensusOverlay {
    world: World<DummyID>,
}

impl CensusOverlay {
    pub fn new(ctx: &mut EventCtx, map: &Map) -> Self {
        let mut world = World::new();
        for (polygon, zone) in map.all_census_zones() {
            let mut draw_normal = GeomBatch::new();
            draw_normal.push(Color::PINK, polygon.to_outline(Distance::meters(5.0)));

            let mut draw_hover = GeomBatch::new();
            draw_hover.push(Color::PINK.alpha(0.5), polygon.clone());

            world
                .add_unnamed()
                .hitbox(polygon.clone())
                .draw(draw_normal)
                .draw_hovered(draw_hover)
                .tooltip(Text::from(format!("{:?}", zone)))
                .build(ctx);
        }
        Self { world }
    }

    pub fn event(&mut self, ctx: &mut EventCtx) {
        // Just trigger tooltips
        self.world.event(ctx);
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.world.draw(g);
    }
}

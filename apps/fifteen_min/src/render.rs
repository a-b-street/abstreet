use map_model::{Building, LaneType};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, RewriteColor};

use crate::isochrone::MovementOptions;
use crate::App;

pub fn draw_star(ctx: &mut EventCtx, b: &Building) -> GeomBatch {
    GeomBatch::load_svg(ctx, "system/assets/tools/star.svg")
        .centered_on(b.polygon.center())
        .color(RewriteColor::ChangeAll(Color::BLACK))
}

pub fn draw_unwalkable_roads(ctx: &mut EventCtx, app: &App) -> Drawable {
    let allow_shoulders = match app.session.movement {
        MovementOptions::Walking(ref opts) => opts.allow_shoulders,
        MovementOptions::Biking => {
            return Drawable::empty(ctx);
        }
    };

    let mut batch = GeomBatch::new();
    'ROADS: for road in app.map.all_roads() {
        if road.is_light_rail() {
            continue;
        }
        for l in &road.lanes {
            if l.lane_type == LaneType::Sidewalk
                || l.lane_type == LaneType::Footway
                || l.lane_type == LaneType::SharedUse
                || (l.lane_type == LaneType::Shoulder && allow_shoulders)
            {
                continue 'ROADS;
            }
        }
        // TODO Skip highways
        batch.push(Color::BLUE.alpha(0.5), road.get_thick_polygon());
    }
    ctx.upload(batch)
}

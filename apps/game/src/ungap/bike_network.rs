use std::collections::HashMap;

use map_model::{LaneType, PathConstraints, Road};
use widgetry::mapspace::DrawUnzoomedShapes;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx};

use crate::app::App;

lazy_static::lazy_static! {
    pub static ref DEDICATED_TRAIL: Color = Color::GREEN;
    pub static ref PROTECTED_BIKE_LANE: Color = Color::hex("#A4DE02");
    pub static ref PAINTED_BIKE_LANE: Color = Color::hex("#76BA1B");
    pub static ref GREENWAY: Color = Color::hex("#4C9A2A");
}

/// Shows the bike network while unzoomed. Handles thickening the roads at low zoom levels.
pub struct DrawNetworkLayer {
    draw_roads: DrawUnzoomedShapes,
    draw_intersections: Drawable,
}

impl DrawNetworkLayer {
    pub fn new(ctx: &EventCtx, app: &App) -> DrawNetworkLayer {
        let mut lines = DrawUnzoomedShapes::builder();
        let mut intersections = HashMap::new();
        for r in app.primary.map.all_roads() {
            let mut bike_lane = false;
            let mut buffer = false;
            for l in &r.lanes {
                if l.lane_type == LaneType::Biking {
                    bike_lane = true;
                } else if matches!(l.lane_type, LaneType::Buffer(_)) {
                    buffer = true;
                }
            }

            let color = if app
                .primary
                .map
                .get_edits()
                .original_roads
                .contains_key(&r.id)
            {
                Color::CYAN
            } else if r.is_cycleway() {
                *DEDICATED_TRAIL
            } else if bike_lane && buffer {
                *PROTECTED_BIKE_LANE
            } else if bike_lane {
                *PAINTED_BIKE_LANE
            } else if is_greenway(r) {
                *GREENWAY
            } else {
                continue;
            };

            lines.add_line(r.center_pts.clone(), r.get_width(), color);

            // Arbitrarily pick a color when two different types of roads meet
            intersections.insert(r.src_i, color);
            intersections.insert(r.dst_i, color);
        }

        let mut batch = GeomBatch::new();
        for (i, color) in intersections {
            // No clear way to thicken the intersection at different zoom levels
            batch.push(color, app.primary.map.get_i(i).polygon.clone());
        }

        DrawNetworkLayer {
            draw_roads: lines.build(),
            draw_intersections: ctx.upload(batch),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw_intersections);
        self.draw_roads.draw(g);
    }
}

// TODO Check how other greenways are tagged.
// https://www.openstreetmap.org/way/262778812 has bicycle=designated, cycleway=shared_lane...
pub fn is_greenway(road: &Road) -> bool {
    !road
        .access_restrictions
        .allow_through_traffic
        .contains(PathConstraints::Car)
        && road
            .access_restrictions
            .allow_through_traffic
            .contains(PathConstraints::Bike)
}

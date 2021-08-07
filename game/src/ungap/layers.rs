use geom::{Circle, Distance, Pt2D};
use map_model::{LaneType, PathConstraints, Road};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, Widget};

use crate::app::App;

lazy_static::lazy_static! {
    pub static ref DEDICATED_TRAIL: Color = Color::GREEN;
    pub static ref PROTECTED_BIKE_LANE: Color = Color::hex("#A4DE02");
    pub static ref PAINTED_BIKE_LANE: Color = Color::hex("#76BA1B");
    pub static ref GREENWAY: Color = Color::hex("#4C9A2A");

    pub static ref EDITED_COLOR: Color = Color::CYAN;
}

pub fn legend(ctx: &mut EventCtx, color: Color, label: &str) -> Widget {
    let radius = 15.0;
    Widget::row(vec![
        GeomBatch::from(vec![(
            color,
            Circle::new(Pt2D::new(radius, radius), Distance::meters(radius)).to_polygon(),
        )])
        .into_widget(ctx)
        .centered_vert(),
        ctx.style()
            .btn_plain
            .text(label)
            .build_def(ctx)
            .centered_vert(),
    ])
}

pub fn render_network_layer(ctx: &mut EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    // The basemap colors are beautiful, but we want to emphasize the bike network, so all's foggy
    // in love and war...
    batch.push(
        Color::BLACK.alpha(0.4),
        app.primary.map.get_boundary_polygon().clone(),
    );

    for r in app.primary.map.all_roads() {
        if r.is_cycleway() {
            batch.push(*DEDICATED_TRAIL, r.get_thick_polygon(&app.primary.map));
            continue;
        }

        if is_greenway(r) {
            batch.push(*GREENWAY, r.get_thick_polygon(&app.primary.map));
        }

        // Don't cover up the arterial/local classification -- add thick side lines to show bike
        // facilties in each direction.
        let mut bike_lane_left = false;
        let mut buffer_left = false;
        let mut bike_lane_right = false;
        let mut buffer_right = false;
        let mut on_left = true;
        for (_, _, lt) in r.lanes_ltr() {
            if lt == LaneType::Driving || lt == LaneType::Bus {
                // We're walking the lanes from left-to-right. So as soon as we hit a vehicle lane,
                // any bike lane we find is on the right side of the road.
                // (Barring really bizarre things like a bike lane in the middle of the road)
                on_left = false;
            } else if lt == LaneType::Biking {
                if on_left {
                    bike_lane_left = true;
                } else {
                    bike_lane_right = true;
                }
            } else if matches!(lt, LaneType::Buffer(_)) {
                if on_left {
                    buffer_left = true;
                } else {
                    buffer_right = true;
                }
            }

            let half_width = r.get_half_width(&app.primary.map);
            for (shift, bike_lane, buffer) in [
                (-1.0, bike_lane_left, buffer_left),
                (1.0, bike_lane_right, buffer_right),
            ] {
                let color = if bike_lane && buffer {
                    *PROTECTED_BIKE_LANE
                } else if bike_lane {
                    *PAINTED_BIKE_LANE
                } else {
                    // If we happen to have a buffer, but no bike lane, let's just not ask
                    // questions...
                    continue;
                };
                if let Ok(pl) = r.center_pts.shift_either_direction(shift * half_width) {
                    batch.push(color, pl.make_polygons(0.9 * half_width));
                }
            }
        }
    }
    batch.upload(ctx)
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

pub fn render_edits(ctx: &mut EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    let map = &app.primary.map;
    for r in &map.get_edits().changed_roads {
        batch.push(
            EDITED_COLOR.alpha(0.5),
            map.get_r(*r).get_thick_polygon(map),
        );
    }
    batch.upload(ctx)
}

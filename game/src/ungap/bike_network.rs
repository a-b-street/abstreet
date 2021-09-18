use std::cell::RefCell;
use std::collections::HashMap;

use geom::{Circle, Distance};
use map_model::{LaneType, PathConstraints, Road};
use widgetry::{Color, Drawable, Fill, GeomBatch, GfxCtx, Texture};

use crate::app::App;

lazy_static::lazy_static! {
    pub static ref DEDICATED_TRAIL: Color = Color::GREEN;
    pub static ref PROTECTED_BIKE_LANE: Color = Color::hex("#A4DE02");
    pub static ref PAINTED_BIKE_LANE: Color = Color::hex("#76BA1B");
    pub static ref GREENWAY: Color = Color::hex("#4C9A2A");
}

/// Shows the bike network while unzoomed. Handles thickening the roads at low zoom levels.
pub struct DrawNetworkLayer {
    per_zoom: RefCell<[Option<Drawable>; 11]>,
}

impl DrawNetworkLayer {
    pub fn new() -> DrawNetworkLayer {
        DrawNetworkLayer {
            per_zoom: Default::default(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        let (zoom, idx) = DrawNetworkLayer::discretize_zoom(g.canvas.cam_zoom);
        let value = &mut self.per_zoom.borrow_mut()[idx];
        if value.is_none() {
            *value = Some(DrawNetworkLayer::render_network_layer(g, app, zoom));
        }
        g.redraw(value.as_ref().unwrap());
    }

    // Continuously changing road width as we zoom looks great, but it's terribly slow. We'd have
    // to move line thickening into the shader to do it better. So recalculate with less
    // granularity. The
    fn discretize_zoom(zoom: f64) -> (f64, usize) {
        if zoom >= 1.0 {
            return (1.0, 10);
        }
        let rounded = (zoom * 10.0).round();
        let idx = rounded as usize;
        (rounded / 10.0, idx)
    }

    fn render_network_layer(g: &mut GfxCtx, app: &App, zoom: f64) -> Drawable {
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;

        // zoom ranges from 0.1 to 1.0
        // Thicker lines as we zoom out. Scale up to 10x. Never shrink past the road's actual width.
        let mut thickness = (1.0 / zoom);
        // And on gigantic maps, zoom may approach 0, so avoid NaNs.
        if !thickness.is_finite() {
            thickness = 10.0;
        }
        println!("at zoom {}, use thickness {}", zoom, thickness);

        let color = Color::hex("#44BC44");

        for r in map.all_roads() {
            let mut bike_lane = false;
            let mut buffer = false;
            for l in &r.lanes {
                if l.lane_type == LaneType::Biking {
                    bike_lane = true;
                } else if matches!(l.lane_type, LaneType::Buffer(_)) {
                    buffer = true;
                }
            }

            // The total road width, scaled up as we zoom out
            let width = thickness * r.get_width();

            if r.is_cycleway() {
                let line_thickness = 1.0 * width;
                let circle_radius = 0.5 * width;
                let shift = line_thickness / 2.0 + 2.0 * circle_radius;

                // Center dash
                batch.push(color, r.center_pts.make_polygons(line_thickness));
                // Dots, both sides
                for side in [1.0, -1.0] {
                    if let Ok(pl) = r.center_pts.shift_either_direction(side * shift) {
                        for (pt, _) in
                            pl.step_along(Distance::meters(thickness * 10.0), Distance::ZERO)
                        {
                            batch.push(color, Circle::new(pt, circle_radius).to_polygon());
                        }
                    }
                }
            } else if bike_lane && buffer {
                // Protected bike lane is three lines
                //
                // 4 thick line
                // 3 space
                // 2 thin line
                //
                // 16
                batch.push(color, r.center_pts.make_polygons((2.0 / 16.0) * width));
                for side in [1.0, -1.0] {
                    if let Ok(pl) = r
                        .center_pts
                        .shift_either_direction(side * (6.0 / 16.0) * width)
                    {
                        batch.push(color, pl.make_polygons((4.0 / 16.0) * width));
                    }
                }
            } else if bike_lane {
                // Painted bike lane is just a solid line
                batch.push(color, r.center_pts.make_polygons(width));
            } else if is_greenway(r) {
                //*GREENWAY
            } else {
                continue;
            };
            // TODO Edits?
        }

        g.upload(batch)
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

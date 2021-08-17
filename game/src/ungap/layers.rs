use std::cell::RefCell;
use std::collections::HashMap;

use geom::Distance;
use map_model::{LaneType, PathConstraints, Road};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx};

use crate::app::App;

lazy_static::lazy_static! {
    pub static ref DEDICATED_TRAIL: Color = Color::GREEN;
    pub static ref PROTECTED_BIKE_LANE: Color = Color::hex("#A4DE02");
    pub static ref PAINTED_BIKE_LANE: Color = Color::hex("#76BA1B");
    pub static ref GREENWAY: Color = Color::hex("#4C9A2A");

    pub static ref EDITED_COLOR: Color = Color::CYAN;
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

    /// Call when the network changes.
    pub fn clear(&mut self) {
        self.per_zoom = Default::default();
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
    // granularity.
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

        // The basemap colors are beautiful, but we want to emphasize the bike network, so all's foggy
        // in love and war...
        batch.push(Color::BLACK.alpha(0.4), map.get_boundary_polygon().clone());

        // Thicker lines as we zoom out. Scale up to 5x. Never shrink past the road's actual width
        let thickness = (0.5 / zoom).max(1.0);

        let mut intersections = HashMap::new();
        for r in map.all_roads() {
            let mut bike_lane = false;
            let mut buffer = false;
            for (_, _, lt) in r.lanes_ltr() {
                if lt == LaneType::Biking {
                    bike_lane = true;
                } else if matches!(lt, LaneType::Buffer(_)) {
                    buffer = true;
                }
            }

            let color = if r.is_cycleway() {
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

            batch.push(
                color,
                r.center_pts.make_polygons(thickness * r.get_width(map)),
            );
            // Arbitrarily pick a color when two different types of roads meet
            intersections.insert(r.src_i, color);
            intersections.insert(r.dst_i, color);
        }

        for (i, color) in intersections {
            // No clear way to thicken the intersection at different zoom levels
            batch.push(color, map.get_i(i).polygon.clone());
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

pub fn render_edits(ctx: &mut EventCtx, app: &App) -> Drawable {
    let mut batch = GeomBatch::new();
    let map = &app.primary.map;
    for r in &map.get_edits().changed_roads {
        batch.extend(
            // dash color
            Color::hex("#F9FF8B"),
            map.get_r(*r).get_thick_polygon(map).to_dashed_outline(
                // thickness
                Distance::meters(3.0),
                // length of each dash
                Distance::meters(5.0),
                // separation between each dash
                Distance::meters(2.0),
            ),
        );
    }
    batch.upload(ctx)
}

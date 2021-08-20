use std::cell::RefCell;
use std::collections::HashMap;

use geom::{Distance, Pt2D};
use map_model::osm;
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Line, Text};

use crate::app::App;

/// Labels roads when unzoomed. Label size and frequency depends on the zoom level.
pub struct DrawRoadLabels {
    per_zoom: RefCell<[Option<Drawable>; 11]>,
}

impl DrawRoadLabels {
    pub fn new() -> DrawRoadLabels {
        DrawRoadLabels {
            per_zoom: Default::default(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        let (zoom, idx) = DrawRoadLabels::discretize_zoom(g.canvas.cam_zoom);
        let value = &mut self.per_zoom.borrow_mut()[idx];
        if value.is_none() {
            *value = Some(DrawRoadLabels::render(g, app, zoom));
        }
        g.redraw(value.as_ref().unwrap());
    }

    fn discretize_zoom(zoom: f64) -> (f64, usize) {
        // TODO Maybe more values between 1.0 and min_zoom_for_detail?
        if zoom >= 1.0 {
            return (1.0, 10);
        }
        let rounded = (zoom * 10.0).round();
        let idx = rounded as usize;
        (rounded / 10.0, idx)
    }

    fn render(g: &mut GfxCtx, app: &App, zoom: f64) -> Drawable {
        let mut batch = GeomBatch::new();
        let map = &app.primary.map;

        // Record the center of each label we place, per road, so we can avoid placing them too
        // close to each other.
        let mut labels_per_road: HashMap<String, Vec<Pt2D>> = HashMap::new();

        let scale_text = 1.0 + 2.0 * (1.0 - zoom);
        //println!("at zoom {}, scale labels {}", zoom, scale_text);

        for r in map.all_roads() {
            if r.get_rank() == osm::RoadRank::Local
                || r.is_light_rail()
                || r.center_pts.length() < Distance::meters(30.0)
            {
                continue;
            }
            let name = if let Some(x) = simplify_name(r.get_name(app.opts.language.as_ref())) {
                x
            } else {
                continue;
            };
            let (pt, angle) = r.center_pts.must_dist_along(r.center_pts.length() / 2.0);

            // Are we too close to some other label?
            let other_pts = labels_per_road.entry(name.clone()).or_insert_with(Vec::new);
            if other_pts
                .iter()
                .any(|other_pt| other_pt.dist_to(pt) < Distance::meters(200.0))
            {
                continue;
            }
            other_pts.push(pt);

            let txt = Text::from(Line(name).fg(Color::WHITE)).bg(Color::BLACK);
            batch.append(
                txt.render_autocropped(g)
                    .scale(scale_text)
                    .centered_on(pt)
                    .rotate_around_batch_center(angle.reorient()),
            );
        }

        g.upload(batch)
    }
}

fn simplify_name(mut x: String) -> Option<String> {
    // Skip unnamed roads and highway exits
    if x == "???" || x.starts_with("Exit for ") {
        return None;
    }

    // TODO Surely somebody has written one of these.

    for (long, short) in [
        ("Northeast", "NE"),
        ("Northwest", "NW"),
        ("Southeast", "SE"),
        ("Southwest", "SW"),
        // Order matters -- do the longer patterns first
        ("North", "N"),
        ("South", "S"),
        ("East", "E"),
        ("West", "W"),
    ] {
        // Only replace directions at the start or end of the string
        x = x.replace(&format!("{} ", long), &format!("{} ", short));
        x = x.replace(&format!(" {}", long), &format!(" {}", short));
    }

    // TODO It's unlikely something will have something like Street capitalized mid-word, but if
    // so, we'll butcher it.
    // Drive -> Dr feels weird
    x = x
        .replace("Street", "St")
        .replace("Boulevard", "Blvd")
        .replace("Avenue", "Ave")
        .replace("Place", "Pl");

    Some(x)
}

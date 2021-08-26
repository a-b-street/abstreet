use std::cell::RefCell;

use aabb_quadtree::QuadTree;
use lazy_static::lazy_static;
use regex::Regex;

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
            debug!("computing DrawRoadLabels(zoom: {}, idx: {})", zoom, idx);
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

        let text_scale = 1.0 + 2.0 * (1.0 - zoom);
        //println!("at zoom {}, scale labels {}", zoom, text_scale);

        let mut non_overlapping = Vec::new();
        let mut quadtree = QuadTree::default(map.get_bounds().as_bbox());

        'ROAD: for r in map.all_roads() {
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

            fn cheaply_overestimate_bounds(
                text: &str,
                text_scale: f64,
                center: Pt2D,
                angle: geom::Angle,
            ) -> geom::Bounds {
                // assume all chars are bigger than largest possible char
                let letter_width = 30.0 * text_scale;
                let letter_height = 30.0 * text_scale;

                geom::Polygon::rectangle_centered(
                    center,
                    Distance::meters(letter_width * text.len() as f64),
                    Distance::meters(letter_height),
                )
                .rotate(angle.reorient())
                .get_bounds()
            }

            // TODO: why all these different reps?
            // TODO: add "buffer" in estimate bounds
            // Don't get too close to other labels.
            let mut search = cheaply_overestimate_bounds(&name, text_scale, pt, angle);
            let bounds_rect = search.get_rectangle();
            search.add_buffer(Distance::meters(1.0));
            for (idx, _, _) in quadtree.query(search.as_bbox()) {
                //  Why is this intersection query necessary? doesn't quadtree.query get close enough?
                if bounds_rect.intersects(&non_overlapping[*idx]) {
                    continue 'ROAD;
                }
            }
            quadtree.insert_with_box(non_overlapping.len(), bounds_rect.get_bounds().as_bbox());
            non_overlapping.push(bounds_rect);

            // No other labels too close - proceed to render text.
            let txt = Text::from(
                Line(&name)
                    .big_heading_plain()
                    .fg(Color::WHITE)
                    .outlined(Color::BLACK),
            );
            let txt_batch = txt
                .render_autocropped(g)
                .scale(text_scale) // <- expensive
                .centered_on(pt) // <- expensive
                .rotate_around_batch_center(angle.reorient()); // <- most expensive

            batch.append(txt_batch);
        }

        g.upload(batch)
    }
}

// TODO Surely somebody has written one of these.
fn simplify_name(mut x: String) -> Option<String> {
    // Skip unnamed roads and highway exits
    if x == "???" || x.starts_with("Exit for ") {
        return None;
    }

    lazy_static! {
        static ref SIMPLIFY_PATTERNS: Vec<(Regex, String)> = simplify_patterns();
    }

    for (search, replace_with) in SIMPLIFY_PATTERNS.iter() {
        // TODO The string copies are probably avoidable...
        x = search.replace(&x, replace_with).to_string();
    }

    Some(x)
}

fn simplify_patterns() -> Vec<(Regex, String)> {
    let mut replace = Vec::new();

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
        replace.push((
            Regex::new(&format!("^{}", long)).unwrap(),
            short.to_string(),
        ));
        replace.push((
            Regex::new(&format!("{}$", long)).unwrap(),
            short.to_string(),
        ));
    }

    for (long, short) in [
        ("Street", "St"),
        ("Boulevard", "Blvd"),
        ("Avenue", "Ave"),
        ("Place", "Pl"),
    ] {
        // At the end is reasonable
        replace.push((
            Regex::new(&format!("{}$", long)).unwrap(),
            short.to_string(),
        ));
        // In the middle, surrounded by spaces
        replace.push((
            Regex::new(&format!(" {} ", long)).unwrap(),
            format!(" {} ", short),
        ));
    }

    replace
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplify_name() {
        for (input, want) in [
            ("Northeast Northgate Way", "NE Northgate Way"),
            ("South 42nd Street", "S 42nd St"),
        ] {
            let got = simplify_name(input.to_string()).unwrap();
            if got != want {
                panic!("simplify_name({}) = {}; expected {}", input, got, want);
            }
        }
    }
}

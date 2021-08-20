use std::cell::RefCell;

use aabb_quadtree::QuadTree;
use lazy_static::lazy_static;
use regex::Regex;

use geom::Distance;
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

        let scale_text = 1.0 + 2.0 * (1.0 - zoom);
        //println!("at zoom {}, scale labels {}", zoom, scale_text);

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

            let txt = Text::from(
                Line(name)
                    .big_heading_plain()
                    .fg(Color::WHITE)
                    .outlined(Color::BLACK),
            );
            let txt_batch = txt
                .render_autocropped(g)
                .scale(scale_text)
                .centered_on(pt)
                .rotate_around_batch_center(angle.reorient());

            // Don't get too close to other labels.
            // TODO This will probably be slower to compute -- we actually render the text before
            // checking hitboxes. :(
            let mut search = txt_batch.get_bounds();
            let poly = search.get_rectangle();
            search.add_buffer(Distance::meters(1.0));
            for (idx, _, _) in quadtree.query(search.as_bbox()) {
                if poly.intersects(&non_overlapping[*idx]) {
                    continue 'ROAD;
                }
            }
            quadtree.insert_with_box(non_overlapping.len(), poly.get_bounds().as_bbox());
            non_overlapping.push(poly);

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

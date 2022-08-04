use std::cell::RefCell;

use aabb_quadtree::QuadTree;
use lazy_static::lazy_static;
use regex::Regex;

use geom::{Angle, Bounds, Distance, Polygon, Pt2D};
use map_model::{osm, Road};
use widgetry::mapspace::PerZoom;
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Line, Text};

use crate::AppLike;

/// Labels roads when unzoomed. Label size and frequency depends on the zoom level.
///
/// By default, the text is white; it works well on dark backgrounds.
pub struct DrawRoadLabels {
    per_zoom: RefCell<Option<PerZoom>>,
    include_roads: Box<dyn Fn(&Road) -> bool>,
    fg_color: Color,
    outline_color: Color,
}

impl DrawRoadLabels {
    /// Label roads that the predicate approves
    pub fn new(include_roads: Box<dyn Fn(&Road) -> bool>) -> Self {
        Self {
            per_zoom: Default::default(),
            include_roads,
            fg_color: Color::WHITE,
            outline_color: Color::BLACK,
        }
    }

    /// Only label major roads
    pub fn only_major_roads() -> Self {
        Self::new(Box::new(|r| {
            r.get_rank() != osm::RoadRank::Local && !r.is_light_rail()
        }))
    }

    pub fn light_background(mut self) -> Self {
        self.fg_color = Color::BLACK;
        self.outline_color = Color::WHITE;
        self
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike) {
        let mut per_zoom = self.per_zoom.borrow_mut();
        if per_zoom.is_none() {
            *per_zoom = Some(PerZoom::new(g.canvas.settings.min_zoom_for_detail, 0.1));
        }
        let per_zoom = per_zoom.as_mut().unwrap();

        let (zoom, idx) = per_zoom.discretize_zoom(g.canvas.cam_zoom);
        let draw = &mut per_zoom.draw_per_zoom[idx];
        if draw.is_none() {
            *draw = Some(self.render(g, app, zoom));
        }
        g.redraw(draw.as_ref().unwrap());
    }

    fn render(&self, g: &mut GfxCtx, app: &dyn AppLike, zoom: f64) -> Drawable {
        let mut batch = GeomBatch::new();
        let map = app.map();

        // We want the effective size of the text to stay around 1
        // effective = zoom * text_scale
        let text_scale = 1.0 / zoom;

        let mut quadtree = QuadTree::default(map.get_bounds().as_bbox());

        'ROAD: for r in map.all_roads() {
            if !(self.include_roads)(r) || r.length() < Distance::meters(30.0) {
                continue;
            }

            let name = if let Some(x) = simplify_name(r.get_name(app.opts().language.as_ref())) {
                x
            } else {
                continue;
            };
            let (pt, angle) = r.center_pts.must_dist_along(r.length() / 2.0);

            // Don't get too close to other labels.
            let big_bounds = cheaply_overestimate_bounds(&name, text_scale, pt, angle);
            if !quadtree.query(big_bounds.as_bbox()).is_empty() {
                continue 'ROAD;
            }
            quadtree.insert_with_box((), big_bounds.as_bbox());

            // No other labels too close - proceed to render text.
            let txt = Text::from(
                Line(&name)
                    .big_heading_plain()
                    .fg(self.fg_color)
                    .outlined(self.outline_color),
            );
            batch.append(txt.render_autocropped(g).multi_transform(
                text_scale,
                pt,
                angle.reorient(),
            ));
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
            Regex::new(&format!("^{long} ")).unwrap(),
            format!("{short} "),
        ));
        replace.push((
            Regex::new(&format!(" {long}$")).unwrap(),
            format!(" {short}"),
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
            ("Northcote Road", "Northcote Road"),
        ] {
            let got = simplify_name(input.to_string()).unwrap();
            if got != want {
                panic!("simplify_name({}) = {}; expected {}", input, got, want);
            }
        }
    }
}

fn cheaply_overestimate_bounds(text: &str, text_scale: f64, center: Pt2D, angle: Angle) -> Bounds {
    // assume all chars are bigger than largest possible char
    let letter_width = 30.0 * text_scale;
    let letter_height = 30.0 * text_scale;

    Polygon::rectangle_centered(
        center,
        Distance::meters(letter_width * text.len() as f64),
        Distance::meters(letter_height),
    )
    .rotate(angle.reorient())
    .get_bounds()
}

/// Draws labels in map-space that roughly fit on the roads and change screen-space size while
/// zooming.
pub struct DrawSimpleRoadLabels {
    draw: RefCell<Option<Drawable>>,
    include_roads: Box<dyn Fn(&Road) -> bool>,
    fg_color: Color,
}

impl DrawSimpleRoadLabels {
    /// Label roads that the predicate approves
    pub fn new(fg_color: Color, include_roads: Box<dyn Fn(&Road) -> bool>) -> Self {
        Self {
            draw: RefCell::new(None),
            include_roads,
            fg_color,
        }
    }

    /// Only label major roads
    pub fn only_major_roads(fg_color: Color) -> Self {
        Self::new(
            fg_color,
            Box::new(|r| r.get_rank() != osm::RoadRank::Local && !r.is_light_rail()),
        )
    }

    pub fn all_roads(fg_color: Color) -> Self {
        Self::new(fg_color, Box::new(|_| true))
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike) {
        let mut draw = self.draw.borrow_mut();
        if draw.is_none() {
            *draw = Some(self.render(g, app));
        }
        g.redraw(draw.as_ref().unwrap());
    }

    fn render(&self, g: &mut GfxCtx, app: &dyn AppLike) -> Drawable {
        let mut batch = GeomBatch::new();
        let map = app.map();

        let mut hitboxes = Vec::new();

        'ROAD: for r in map.all_roads() {
            if !(self.include_roads)(r) || r.length() < Distance::meters(30.0) {
                continue;
            }

            let name = if let Some(x) = simplify_name(r.get_name(app.opts().language.as_ref())) {
                x
            } else {
                continue;
            };
            let (pt, angle) = r.center_pts.must_dist_along(r.length() / 2.0);

            // TODO Ideally we make the text height always be a bit less than the road width. I'm
            // having lots of trouble relating the two, so do something manually tuned for the
            // moment.
            let width = r.get_width();
            let scale = if width < Distance::meters(8.0) {
                0.3
            } else if width < Distance::meters(6.0) {
                0.2
            } else {
                0.5
            };

            let txt = Text::from(Line(&name).fg(self.fg_color));
            let txt_batch = txt.render_autocropped(g).scale(scale);
            let rect = txt_batch
                .get_bounds()
                .get_rectangle()
                .centered_on(pt)
                .rotate_around(angle.reorient(), pt);
            for x in &hitboxes {
                if rect.intersects(x) {
                    continue 'ROAD;
                }
            }
            hitboxes.push(rect);

            batch.append(
                txt_batch
                    .centered_on(pt)
                    .rotate_around_batch_center(angle.reorient()),
            );
        }

        g.upload(batch)
    }
}

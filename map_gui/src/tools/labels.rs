use std::cell::RefCell;

use aabb_quadtree::QuadTree;
use lazy_static::lazy_static;
use regex::Regex;

use abstutil::Timer;
use geom::{Angle, Bounds, Distance, Polygon, Pt2D};
use map_model::{osm, Road};
use widgetry::mapspace::PerZoom;
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, Text};

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

/// Draws labels in map-space that roughly fit on the roads. Don't change behavior during zooming;
/// labels are only meant to be legible when zoomed in.
pub struct DrawSimpleRoadLabels {
    draw: Drawable,
    include_roads: Box<dyn Fn(&Road) -> bool>,
    fg_color: Color,
}

impl DrawSimpleRoadLabels {
    /// Label roads that the predicate approves
    pub fn new(
        ctx: &mut EventCtx,
        app: &dyn AppLike,
        fg_color: Color,
        include_roads: Box<dyn Fn(&Road) -> bool>,
    ) -> Self {
        let mut labels = Self {
            draw: Drawable::empty(ctx),
            include_roads,
            fg_color,
        };
        ctx.loading_screen("label roads", |ctx, timer| {
            labels.draw = labels.render(ctx, app, timer);
        });
        labels
    }

    pub fn empty(ctx: &EventCtx) -> Self {
        Self {
            draw: Drawable::empty(ctx),
            include_roads: Box::new(|_| false),
            fg_color: Color::CLEAR,
        }
    }

    /// Only label major roads
    pub fn only_major_roads(ctx: &mut EventCtx, app: &dyn AppLike, fg_color: Color) -> Self {
        Self::new(
            ctx,
            app,
            fg_color,
            Box::new(|r| r.get_rank() != osm::RoadRank::Local && !r.is_light_rail()),
        )
    }

    pub fn all_roads(ctx: &mut EventCtx, app: &dyn AppLike, fg_color: Color) -> Self {
        Self::new(ctx, app, fg_color, Box::new(|_| true))
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.redraw(&self.draw);
    }

    fn render(&self, ctx: &mut EventCtx, app: &dyn AppLike, timer: &mut Timer) -> Drawable {
        let mut batch = GeomBatch::new();
        let map = app.map();

        let mut hitboxes = Vec::new();

        timer.start_iter("render roads", map.all_roads().len());
        'ROAD: for r in map.all_roads() {
            timer.next();
            if !(self.include_roads)(r) || r.length() < Distance::meters(30.0) {
                continue;
            }

            let name = if let Some(x) = simplify_name(r.get_name(app.opts().language.as_ref())) {
                x
            } else {
                continue;
            };
            let (pt, angle) = r.center_pts.must_dist_along(r.length() / 2.0);

            let txt = Text::from(Line(&name).fg(self.fg_color));
            let txt_batch = txt.render_autocropped(ctx);
            let txt_bounds = txt_batch.get_bounds();
            if txt_batch.is_empty() {
                // This happens when we don't have a font loaded with the right characters
                continue;
            }

            // The road has an outline, so leave a slight buffer
            let road_width = (0.9 * r.get_width()).inner_meters();
            // Also a buffer from both ends of the road
            let road_length = (0.9 * r.length()).inner_meters();

            // Fit the text height in the road width perfectly
            let mut scale = road_width / txt_bounds.height();

            info!(
                "{name}: txt height {}, road width {}, so scale {}",
                txt_bounds.height(),
                road_width,
                scale
            );

            // If the road is short and we'll overflow, then scale down even more.
            if txt_bounds.width() * scale > road_length {
                scale = road_length / txt_bounds.width();
                info!("  overflowing width, so actually scale {scale}");
            }

            let txt_batch = txt_batch.scale(scale);

            // TODO If the labels fit in the road polygon by construction, then we can scrap hitbox
            // testing entirely
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

        ctx.upload(batch)
    }
}

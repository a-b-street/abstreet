use geom::{Angle, Bounds, GPSBounds, Polygon, Pt2D};

use crate::{
    svg, Color, DeferDraw, Drawable, EventCtx, Fill, GfxCtx, JustDraw, Prerender, ScreenDims,
    Widget,
};

pub mod geom_batch_stack;

/// A mutable builder for a group of colored polygons.
#[derive(Clone)]
pub struct GeomBatch {
    // f64 is the z-value offset. This must be in (-1, 0], with values closer to -1.0
    // rendering above values closer to 0.0.
    pub(crate) list: Vec<(Fill, Polygon, f64)>,
    pub autocrop_dims: bool,
}

impl std::fmt::Debug for GeomBatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeomBatch")
            .field("bounds", &self.get_bounds())
            .field("items", &self.list.len())
            .field("autocrop_dims", &self.autocrop_dims)
            .finish()
    }
}

impl GeomBatch {
    /// Creates an empty batch.
    pub fn new() -> GeomBatch {
        GeomBatch {
            list: Vec::new(),
            autocrop_dims: true,
        }
    }

    /// Adds a single polygon, painted according to `Fill`
    pub fn push<F: Into<Fill>>(&mut self, fill: F, p: Polygon) {
        self.push_with_z(fill, p, 0.0);
    }

    /// Offset z value to render above/below other polygons.
    /// z must be in (-1, 0] to ensure we don't traverse layers of the UI - to make
    /// sure we don't inadvertently render something *above* a tooltip, etc.
    pub fn push_with_z<F: Into<Fill>>(&mut self, fill: F, p: Polygon, z_offset: f64) {
        debug_assert!(z_offset > -1.0);
        debug_assert!(z_offset <= 0.0);
        self.list.push((fill.into(), p, z_offset));
    }

    /// Adds a single polygon to the front of the batch, painted according to `Fill`
    pub fn unshift<F: Into<Fill>>(&mut self, fill: F, p: Polygon) {
        self.list.insert(0, (fill.into(), p, 0.0));
    }

    /// Applies one Fill to many polygons.
    pub fn extend<F: Into<Fill>>(&mut self, fill: F, polys: Vec<Polygon>) {
        let fill = fill.into();
        for p in polys {
            self.list.push((fill.clone(), p, 0.0));
        }
    }

    /// Appends all colored polygons from another batch to the current one.
    pub fn append(&mut self, other: GeomBatch) {
        self.list.extend(other.list);
    }

    /// Returns the colored polygons in this batch, destroying the batch.
    pub fn consume(self) -> Vec<(Fill, Polygon, f64)> {
        self.list
    }

    /// Draws the batch, consuming it. Only use this for drawing things once.
    pub fn draw(self, g: &mut GfxCtx) {
        let obj = g.prerender.upload_temporary(self);
        g.redraw(&obj);
    }

    /// Upload the batch of polygons to the GPU, returning something that can be cheaply redrawn
    /// many times later.
    pub fn upload(self, ctx: &EventCtx) -> Drawable {
        ctx.prerender.upload(self)
    }

    /// Wrap in a Widget for layouting, so this batch can become part of a larger one.
    pub fn batch(self) -> Widget {
        DeferDraw::new_widget(self)
    }

    /// Wrap in a Widget, so the batch can be drawn as part of a Panel.
    pub fn into_widget(self, ctx: &EventCtx) -> Widget {
        JustDraw::wrap(ctx, self)
    }

    /// Compute the bounds of all polygons in this batch.
    pub fn get_bounds(&self) -> Bounds {
        let mut bounds = Bounds::new();
        for (_, poly, _) in &self.list {
            bounds.union(poly.get_bounds());
        }
        if !self.autocrop_dims {
            bounds.update(Pt2D::new(0.0, 0.0));
        }
        bounds
    }

    /// Sets the top-left to 0, 0. Not sure exactly when this should be used.
    pub fn autocrop(mut self) -> GeomBatch {
        let bounds = self.get_bounds();
        if bounds.min_x == 0.0 && bounds.min_y == 0.0 {
            return self;
        }
        for (_, poly, _) in &mut self.list {
            *poly = poly.translate(-bounds.min_x, -bounds.min_y);
        }
        self
    }

    /// Builds a single polygon covering everything in this batch. Use to create a hitbox.
    pub fn unioned_polygon(&self) -> Polygon {
        let mut result = self.list[0].1.clone();
        for (_, p, _) in &self.list[1..] {
            result = result.union(p.clone());
        }
        result
    }

    /// True when the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Returns the width and height of all geometry contained in the batch.
    pub fn get_dims(&self) -> ScreenDims {
        // TODO Maybe warn about this happening and avoid in the first place? Sometimes we wind up
        // trying to draw completely empty text.
        if self.is_empty() {
            return ScreenDims::new(0.0, 0.0);
        }
        let bounds = self.get_bounds();
        ScreenDims::new(bounds.width(), bounds.height())
    }

    /// Returns a batch containing an SVG from a file.
    pub fn load_svg<P: AsRef<Prerender>, I: AsRef<str>>(prerender: &P, filename: I) -> GeomBatch {
        svg::load_svg(prerender.as_ref(), filename.as_ref()).0
    }

    /// Returns a GeomBatch from the bytes of a utf8 encoded SVG string.
    pub fn load_svg_bytes<P: AsRef<Prerender>>(
        prerender: &P,
        labeled_bytes: (&str, &[u8]),
    ) -> GeomBatch {
        svg::load_svg_bytes(prerender.as_ref(), labeled_bytes.0, labeled_bytes.1)
            .expect("invalid svg bytes")
            .0
    }

    /// Returns a GeomBatch from the bytes of a utf8 encoded SVG string.
    ///
    /// Prefer to use `load_svg_bytes`, which caches the parsed SVG, unless
    /// the SVG was dynamically generated, or is otherwise unlikely to be
    /// reused.
    pub fn load_svg_bytes_uncached(raw: &[u8]) -> GeomBatch {
        svg::load_svg_from_bytes_uncached(raw).unwrap().0
    }

    /// Transforms all colors in a batch.
    pub fn color(mut self, transformation: RewriteColor) -> GeomBatch {
        for (fancy, _, _) in &mut self.list {
            if let Fill::Color(ref mut c) = fancy {
                *c = transformation.apply(*c);
            }
        }
        self
    }

    /// Translates the batch to be centered on some point.
    pub fn centered_on(self, center: Pt2D) -> GeomBatch {
        let dims = self.get_dims();
        let dx = center.x() - dims.width / 2.0;
        let dy = center.y() - dims.height / 2.0;
        self.translate(dx, dy)
    }

    /// Translates the batch by some offset.
    pub fn translate(mut self, dx: f64, dy: f64) -> GeomBatch {
        for (_, poly, _) in &mut self.list {
            *poly = poly.translate(dx, dy);
        }
        self
    }

    /// Rotates each polygon in the batch relative to the center of that polygon.
    pub fn rotate(mut self, angle: Angle) -> GeomBatch {
        for (_, poly, _) in &mut self.list {
            *poly = poly.rotate(angle);
        }
        self
    }

    /// Rotates each polygon in the batch relative to the center of the entire batch.
    pub fn rotate_around_batch_center(mut self, angle: Angle) -> GeomBatch {
        let center = self.get_bounds().center();
        for (_, poly, _) in &mut self.list {
            *poly = poly.rotate_around(angle, center);
        }
        self
    }

    /// Scales the batch by some factor.
    pub fn scale(self, factor: f64) -> GeomBatch {
        self.scale_xy(factor, factor)
    }

    pub fn scale_xy(mut self, x_factor: f64, y_factor: f64) -> GeomBatch {
        #[allow(clippy::float_cmp)]
        if x_factor == 1.0 && y_factor == 1.0 {
            return self;
        }

        for (_, poly, _) in &mut self.list {
            // strip_rings first -- sometimes when scaling down, the original rings collapse. Since
            // this polygon is part of a GeomBatch anyway, not calling to_outline on it.
            *poly = poly.strip_rings().scale_xy(x_factor, y_factor);
        }
        self
    }

    /// Overrides the Z-ordering offset for the batch. Must be in (-1, 0], with values closer to -1
    /// rendering on top.
    pub fn set_z_offset(mut self, offset: f64) -> GeomBatch {
        if offset <= -1.0 || offset > 0.0 {
            panic!("set_z_offset({}) must be in (-1, 0]", offset);
        }
        for (_, _, z) in &mut self.list {
            *z = offset;
        }
        self
    }

    /// Exports the batch to a list of GeoJSON features, labeling each colored polygon. Z-values,
    /// alpha values from the color, and non-RGB fill patterns are lost. If the polygon isn't a
    /// ring, it's skipped. The world-space coordinates are optionally translated back to GPS.
    pub fn into_geojson(self, gps_bounds: Option<&GPSBounds>) -> Vec<geojson::Feature> {
        let mut features = Vec::new();
        for (fill, polygon, _) in self.list {
            if let Fill::Color(color) = fill {
                let mut properties = serde_json::Map::new();
                properties.insert("color".to_string(), color.as_hex().into());
                features.push(geojson::Feature {
                    bbox: None,
                    geometry: Some(polygon.to_geojson(gps_bounds)),
                    id: None,
                    properties: Some(properties),
                    foreign_members: None,
                });
            }
        }
        features
    }
}

impl<F: Into<Fill>> From<Vec<(F, Polygon)>> for GeomBatch {
    /// Creates a batch of filled polygons.
    fn from(list: Vec<(F, Polygon)>) -> GeomBatch {
        GeomBatch {
            list: list.into_iter().map(|(c, p)| (c.into(), p, 0.0)).collect(),
            autocrop_dims: true,
        }
    }
}

/// A way to transform all colors in a GeomBatch.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RewriteColor {
    /// Don't do anything
    NoOp,
    /// Change every instance of the first color to the second
    Change(Color, Color),
    /// Change all colors to the specified value. For this to be interesting, the batch shouldn't
    /// be a solid block of color. This does not modify Color::CLEAR.
    ChangeAll(Color),
    /// Change the alpha value of all colors to this value.
    ChangeAlpha(f32),
    /// Convert all colors to greyscale.
    MakeGrayscale,
}

impl std::convert::From<Color> for RewriteColor {
    fn from(color: Color) -> RewriteColor {
        RewriteColor::ChangeAll(color)
    }
}

impl RewriteColor {
    fn apply(&self, c: Color) -> Color {
        match self {
            RewriteColor::NoOp => c,
            RewriteColor::Change(from, to) => {
                if c == *from {
                    *to
                } else {
                    c
                }
            }
            RewriteColor::ChangeAll(to) => {
                if c == Color::CLEAR {
                    c
                } else {
                    *to
                }
            }
            RewriteColor::ChangeAlpha(alpha) => c.alpha(*alpha),
            RewriteColor::MakeGrayscale => {
                let avg = (c.r + c.g + c.b) / 3.0;
                Color::grey(avg).alpha(c.a)
            }
        }
    }
}

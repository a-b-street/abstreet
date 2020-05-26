use crate::{svg, Color, Drawable, EventCtx, FancyColor, GfxCtx, Prerender, ScreenDims};
use geom::{Angle, Bounds, Polygon, Pt2D};

/// A mutable builder for a group of colored polygons.
#[derive(Clone)]
pub struct GeomBatch {
    pub(crate) list: Vec<(FancyColor, Polygon)>,
    pub autocrop_dims: bool,
}

impl GeomBatch {
    /// Creates an empty batch.
    pub fn new() -> GeomBatch {
        GeomBatch {
            list: Vec::new(),
            autocrop_dims: true,
        }
    }

    /// Creates a batch of colored polygons.
    pub fn from(list: Vec<(Color, Polygon)>) -> GeomBatch {
        GeomBatch {
            list: list
                .into_iter()
                .map(|(c, p)| (FancyColor::RGBA(c), p))
                .collect(),
            autocrop_dims: true,
        }
    }

    /// Adds a single colored polygon.
    pub fn push(&mut self, color: Color, p: Polygon) {
        self.list.push((FancyColor::RGBA(color), p));
    }
    pub fn fancy_push(&mut self, color: FancyColor, p: Polygon) {
        self.list.push((color, p));
    }

    /// Applies one color to many polygons.
    pub fn extend(&mut self, color: Color, polys: Vec<Polygon>) {
        for p in polys {
            self.list.push((FancyColor::RGBA(color), p));
        }
    }

    /// Appends all colored polygons from another batch to the current one.
    pub fn append(&mut self, other: GeomBatch) {
        self.list.extend(other.list);
    }

    /// Returns the colored polygons in this batch, destroying the batch.
    pub fn consume(self) -> Vec<(FancyColor, Polygon)> {
        self.list
    }

    /// Draws the batch, consuming it. Only use this for drawing things once.
    pub fn draw(self, g: &mut GfxCtx) {
        let refs = self
            .list
            .iter()
            .map(|(color, p)| (color.clone(), p))
            .collect();
        let obj = g.prerender.upload_temporary(refs);
        g.redraw(&obj);
    }

    /// Upload the batch of polygons to the GPU, returning something that can be cheaply redrawn
    /// many times later.
    pub fn upload(self, ctx: &EventCtx) -> Drawable {
        ctx.prerender.upload(self)
    }

    /// Sets the top-left to 0, 0. Not sure exactly when this should be used.
    pub fn autocrop(mut self) -> GeomBatch {
        let mut bounds = Bounds::new();
        for (_, poly) in &self.list {
            bounds.union(poly.get_bounds());
        }
        if bounds.min_x == 0.0 && bounds.min_y == 0.0 {
            return self;
        }
        for (_, poly) in &mut self.list {
            *poly = poly.translate(-bounds.min_x, -bounds.min_y);
        }
        self
    }

    /// True when the batch is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Returns the width and height of all geometry contained in the batch.
    pub fn get_dims(&self) -> ScreenDims {
        // TODO Maybe warn about this happening and avoid in the first place? Sometimes we wind up
        // trying to draw completely empty text.
        if self.is_empty() {
            return ScreenDims::new(0.0, 0.0);
        }
        let mut bounds = Bounds::new();
        for (_, poly) in &self.list {
            bounds.union(poly.get_bounds());
        }
        if self.autocrop_dims {
            ScreenDims::new(bounds.width(), bounds.height())
        } else {
            ScreenDims::new(bounds.max_x, bounds.max_y)
        }
    }

    /// Transforms all colors in a batch.
    pub fn rewrite_color(&mut self, transformation: RewriteColor) {
        for (fancy, _) in self.list.iter_mut() {
            if let FancyColor::RGBA(ref mut c) = fancy {
                *c = transformation.apply(*c);
            }
        }
    }

    // TODO Weird API.
    /// Adds an SVG image to the current batch, applying the transformations first.
    pub fn add_svg(
        &mut self,
        prerender: &Prerender,
        filename: &str,
        center: Pt2D,
        scale: f64,
        rotate: Angle,
        rewrite: RewriteColor,
        map_space: bool,
    ) {
        self.add_transformed(
            svg::load_svg(
                prerender,
                filename,
                if map_space {
                    1.0
                } else {
                    *prerender.assets.scale_factor.borrow()
                },
            )
            .0,
            center,
            scale,
            rotate,
            rewrite,
        );
    }

    /// Parse an SVG string and add it to the batch.
    pub fn add_svg_contents(&mut self, raw: Vec<u8>) {
        let svg_tree = usvg::Tree::from_data(&raw, &usvg::Options::default()).unwrap();
        svg::add_svg_inner(self, svg_tree, svg::HIGH_QUALITY, 1.0).unwrap();
    }

    /// Adds geometry from another batch to the current batch, first centering it on the given
    /// point.
    pub fn add_centered(&mut self, other: GeomBatch, center: Pt2D) {
        self.add_transformed(other, center, 1.0, Angle::ZERO, RewriteColor::NoOp);
    }

    /// Adds geometry from another batch to the current batch, first transforming it. The
    /// translation centers on the given point.
    pub fn add_transformed(
        &mut self,
        other: GeomBatch,
        center: Pt2D,
        scale: f64,
        rotate: Angle,
        rewrite: RewriteColor,
    ) {
        let dims = other.get_dims();
        let dx = center.x() - dims.width * scale / 2.0;
        let dy = center.y() - dims.height * scale / 2.0;
        for (mut fancy_color, mut poly) in other.consume() {
            // Avoid unnecessary transformations for slight perf boost
            if scale != 1.0 {
                poly = poly.scale(scale);
            }
            poly = poly.translate(dx, dy);
            if rotate != Angle::ZERO {
                poly = poly.rotate(rotate);
            }
            if let FancyColor::RGBA(ref mut c) = fancy_color {
                *c = rewrite.apply(*c);
            }
            self.fancy_push(fancy_color, poly);
        }
    }

    // TODO Weird API
    /// Adds geometry from another batch to the current batch, first translating it.
    pub fn add_translated(&mut self, other: GeomBatch, dx: f64, dy: f64) {
        for (color, poly) in other.consume() {
            self.fancy_push(color, poly.translate(dx, dy));
        }
    }

    /// Scales the batch by some factor.
    pub fn scale(mut self, factor: f64) -> GeomBatch {
        for (_, poly) in &mut self.list {
            *poly = poly.scale(factor);
        }
        self
    }
}

pub enum RewriteColor {
    NoOp,
    Change(Color, Color),
    ChangeMore(Vec<(Color, Color)>),
    ChangeAll(Color),
    ChangeAlpha(f32),
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
            RewriteColor::ChangeMore(ref list) => {
                for (from, to) in list {
                    if c == *from {
                        return *to;
                    }
                }
                c
            }
            RewriteColor::ChangeAll(to) => *to,
            RewriteColor::ChangeAlpha(alpha) => c.alpha(*alpha),
        }
    }
}

use crate::{svg, Color, Drawable, EventCtx, FancyColor, GfxCtx, Prerender, ScreenDims};
use geom::{Angle, Bounds, Polygon, Pt2D};

/// A mutable builder for a group of colored polygons.
#[derive(Clone)]
pub struct GeomBatch {
    pub(crate) list: Vec<(FancyColor, Polygon)>,
    // TODO A weird hack for text.
    pub(crate) dims_text: bool,
}

impl GeomBatch {
    /// Creates an empty batch.
    pub fn new() -> GeomBatch {
        GeomBatch {
            list: Vec::new(),
            dims_text: false,
        }
    }

    /// Creates a batch of colored polygons.
    pub fn from(list: Vec<(Color, Polygon)>) -> GeomBatch {
        GeomBatch {
            list: list
                .into_iter()
                .map(|(c, p)| (FancyColor::Plain(c), p))
                .collect(),
            dims_text: false,
        }
    }

    /// Adds a single colored polygon.
    pub fn push(&mut self, color: Color, p: Polygon) {
        self.list.push((FancyColor::Plain(color), p));
    }
    pub fn fancy_push(&mut self, color: FancyColor, p: Polygon) {
        self.list.push((color, p));
    }

    /// Applies one color to many polygons.
    pub fn extend(&mut self, color: Color, polys: Vec<Polygon>) {
        for p in polys {
            self.list.push((FancyColor::Plain(color), p));
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
    pub(crate) fn autocrop(mut self) -> GeomBatch {
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
        if self.dims_text {
            ScreenDims::new(bounds.max_x, bounds.max_y)
        } else {
            ScreenDims::new(bounds.width(), bounds.height())
        }
    }

    /// Transforms all colors in a batch.
    pub fn rewrite_color(&mut self, transformation: RewriteColor) {
        for (fancy, _) in self.list.iter_mut() {
            if let FancyColor::Plain(ref mut c) = fancy {
                match transformation {
                    RewriteColor::NoOp => {}
                    RewriteColor::Change(from, to) => {
                        if *c == from {
                            *c = to;
                        }
                    }
                    RewriteColor::ChangeAll(to) => {
                        *c = to;
                    }
                }
            }
        }
    }

    // TODO Weird API.
    /// Creates a new batch containing an SVG image, also returning the bounds of the SVG. The
    /// dimensions come from the SVG image size -- if the image has blank padding on the right and
    /// bottom side, this is captured by the bounds.
    pub fn from_svg<I: Into<String>>(
        ctx: &EventCtx,
        path: I,
        rewrite: RewriteColor,
    ) -> (GeomBatch, Bounds) {
        let (mut batch, bounds) = svg::load_svg(ctx.prerender, &path.into());
        batch.rewrite_color(rewrite);
        (batch, bounds)
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
    ) {
        self.add_transformed(svg::load_svg(prerender, filename).0, center, scale, rotate);
    }

    /// Adds geometry from another batch to the current batch, first transforming it. The
    /// translation centers on the given point.
    pub fn add_transformed(&mut self, other: GeomBatch, center: Pt2D, scale: f64, rotate: Angle) {
        let dims = other.get_dims();
        let dx = center.x() - dims.width * scale / 2.0;
        let dy = center.y() - dims.height * scale / 2.0;
        for (color, mut poly) in other.consume() {
            // Avoid unnecessary transformations for slight perf boost
            if scale != 1.0 {
                poly = poly.scale(scale);
            }
            poly = poly.translate(dx, dy);
            if rotate != Angle::ZERO {
                poly = poly.rotate(rotate);
            }
            self.fancy_push(color, poly);
        }
    }

    // TODO Weird API
    /// Adds geometry from another batch to the current batch, first translating it.
    pub fn add_translated(&mut self, other: GeomBatch, dx: f64, dy: f64) {
        for (color, poly) in other.consume() {
            self.fancy_push(color, poly.translate(dx, dy));
        }
    }
}

pub enum RewriteColor {
    NoOp,
    Change(Color, Color),
    ChangeAll(Color),
}

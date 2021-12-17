mod unzoomed;
mod world;

use geom::Polygon;

use crate::{Drawable, EventCtx, Fill, GeomBatch, GfxCtx, RewriteColor};
pub use unzoomed::DrawUnzoomedShapes;
pub use world::{DummyID, ObjectID, World, WorldOutcome};

/// Draws one of two versions of something, based on whether the canvas is zoomed in past a threshold.
pub struct ToggleZoomed {
    // Some callers access directly for minimaps
    pub unzoomed: Drawable,
    pub zoomed: Drawable,
    // Draw the same thing whether zoomed or unzoomed
    always_draw_unzoomed: bool,
}

impl ToggleZoomed {
    pub fn new(ctx: &EventCtx, unzoomed: GeomBatch, zoomed: GeomBatch) -> ToggleZoomed {
        ToggleZoomed {
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
            always_draw_unzoomed: false,
        }
    }

    pub fn empty(ctx: &EventCtx) -> ToggleZoomed {
        ToggleZoomed {
            unzoomed: Drawable::empty(ctx),
            zoomed: Drawable::empty(ctx),
            always_draw_unzoomed: false,
        }
    }

    pub fn builder() -> ToggleZoomedBuilder {
        ToggleZoomedBuilder {
            unzoomed: GeomBatch::new(),
            zoomed: GeomBatch::new(),
            always_draw_unzoomed: false,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        if self.always_draw_unzoomed || g.canvas.cam_zoom < g.canvas.settings.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
}

#[derive(Clone)]
pub struct ToggleZoomedBuilder {
    pub unzoomed: GeomBatch,
    pub zoomed: GeomBatch,
    always_draw_unzoomed: bool,
}

impl ToggleZoomedBuilder {
    /// Transforms all colors in both batches.
    pub fn color(mut self, transformation: RewriteColor) -> Self {
        self.unzoomed = self.unzoomed.color(transformation);
        if !self.always_draw_unzoomed {
            self.zoomed = self.zoomed.color(transformation);
        }
        self
    }

    /// Adds a single polygon to both batches, painted according to `Fill`
    pub fn push<F: Into<Fill>>(mut self, fill: F, p: Polygon) -> Self {
        let fill = fill.into();
        if !self.always_draw_unzoomed {
            self.zoomed.push(fill.clone(), p.clone());
        }
        self.unzoomed.push(fill, p);
        self
    }

    /// Mark that this object will be drawn differently when zoomed and unzoomed, undoing the
    /// effects of converting from a single `GeomBatch`. Idempotent.
    pub fn draw_differently_zoomed(mut self) -> Self {
        if self.always_draw_unzoomed {
            self.always_draw_unzoomed = false;
            self.zoomed = self.unzoomed.clone();
        }
        self
    }

    pub fn build(self, ctx: &EventCtx) -> ToggleZoomed {
        if self.always_draw_unzoomed {
            assert!(self.zoomed.is_empty());
        }
        ToggleZoomed {
            unzoomed: ctx.upload(self.unzoomed),
            zoomed: ctx.upload(self.zoomed),
            always_draw_unzoomed: self.always_draw_unzoomed,
        }
    }
}

// Drawing just one batch means the same thing will appear whether zoomed or unzoomed
impl std::convert::From<GeomBatch> for ToggleZoomedBuilder {
    fn from(unzoomed: GeomBatch) -> Self {
        Self {
            unzoomed,
            zoomed: GeomBatch::new(),
            always_draw_unzoomed: true,
        }
    }
}

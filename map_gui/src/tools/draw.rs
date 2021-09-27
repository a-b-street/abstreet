use widgetry::{Drawable, EventCtx, GeomBatch, GfxCtx};

use crate::AppLike;

/// Draws one of two versions of something, based on whether the app is zoomed in past a threshold.
pub struct ToggleZoomed {
    // Some callers access directly for minimaps
    pub unzoomed: Drawable,
    pub zoomed: Drawable,
}

impl ToggleZoomed {
    pub fn new(ctx: &EventCtx, unzoomed: GeomBatch, zoomed: GeomBatch) -> ToggleZoomed {
        ToggleZoomed {
            unzoomed: ctx.upload(unzoomed),
            zoomed: ctx.upload(zoomed),
        }
    }

    pub fn empty(ctx: &EventCtx) -> ToggleZoomed {
        ToggleZoomed {
            unzoomed: Drawable::empty(ctx),
            zoomed: Drawable::empty(ctx),
        }
    }

    pub fn builder() -> ToggleZoomedBuilder {
        ToggleZoomedBuilder {
            unzoomed: GeomBatch::new(),
            zoomed: GeomBatch::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike) {
        if g.canvas.cam_zoom < app.opts().min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }
}

pub struct ToggleZoomedBuilder {
    pub unzoomed: GeomBatch,
    pub zoomed: GeomBatch,
}

impl ToggleZoomedBuilder {
    pub fn build(self, ctx: &EventCtx) -> ToggleZoomed {
        ToggleZoomed::new(ctx, self.unzoomed, self.zoomed)
    }
}

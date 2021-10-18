use std::cell::RefCell;

use geom::{Distance, PolyLine};

use crate::{Color, Drawable, GeomBatch, GfxCtx};

/// Draws a `PolyLine` with constant screen-space thickness, no matter how much the canvas is
/// unzoomed.
///
/// ... But not yet. As an approximation of that, just discretize zoom into 10 buckets. Also,
/// specify the behavior when barely unzoomed or zoomed in -- the line starts being drawn in
/// map-space "normally" without a constant screen-space width.
pub struct UnzoomedLines {
    lines: Vec<UnzoomedLine>,
    per_zoom: RefCell<[Option<Drawable>; 11]>,
}

struct UnzoomedLine {
    polyline: PolyLine,
    width: Distance,
    color: Color,
}

pub struct UnzoomedLinesBuilder {
    lines: Vec<UnzoomedLine>,
}

impl UnzoomedLines {
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            per_zoom: Default::default(),
        }
    }

    pub fn builder() -> UnzoomedLinesBuilder {
        UnzoomedLinesBuilder { lines: Vec::new() }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let (zoom, idx) = discretize_zoom(g.canvas.cam_zoom);
        let value = &mut self.per_zoom.borrow_mut()[idx];
        if value.is_none() {
            *value = Some(render_thick_lines(g, &self.lines, zoom));
        }
        g.redraw(value.as_ref().unwrap());
    }
}

impl UnzoomedLinesBuilder {
    pub fn add(&mut self, polyline: PolyLine, width: Distance, color: Color) {
        self.lines.push(UnzoomedLine {
            polyline,
            width,
            color,
        });
    }

    // TODO We might take EventCtx here to upload something to the GPU.
    pub fn build(self) -> UnzoomedLines {
        UnzoomedLines {
            lines: self.lines,
            per_zoom: Default::default(),
        }
    }
}

// Continuously changing road width as we zoom looks great, but it's terribly slow. We'd have to
// move line thickening into the shader to do it better. So recalculate with less granularity.
fn discretize_zoom(zoom: f64) -> (f64, usize) {
    if zoom >= 1.0 {
        return (1.0, 10);
    }
    let rounded = (zoom * 10.0).round();
    let idx = rounded as usize;
    (rounded / 10.0, idx)
}

fn render_thick_lines(g: &mut GfxCtx, lines: &[UnzoomedLine], zoom: f64) -> Drawable {
    // Thicker lines as we zoom out. Scale up to 5x. Never shrink past the original width.
    let mut thickness = (0.5 / zoom).max(1.0);
    // And on gigantic maps, zoom may approach 0, so avoid NaNs.
    if !thickness.is_finite() {
        thickness = 5.0;
    }

    let mut batch = GeomBatch::new();
    for line in lines {
        batch.push(
            line.color,
            line.polyline.make_polygons(thickness * line.width),
        );
    }
    g.upload(batch)
}

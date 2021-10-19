use std::cell::RefCell;

use geom::{Circle, Distance, PolyLine, Pt2D};

use crate::{Color, Drawable, GeomBatch, GfxCtx};

/// Draw `Circles` and `PolyLines` in map-space that scale their size as the canvas is zoomed. The
/// goal is to appear with roughly constant screen-space size, but for the moment, this is
/// approximated by discretizing into 10 buckets. The scaling only happens when the canvas is
/// zoomed out less than a value of 1.0.
pub struct DrawUnzoomedShapes {
    lines: Vec<UnzoomedLine>,
    circles: Vec<UnzoomedCircle>,
    per_zoom: RefCell<[Option<Drawable>; 11]>,
}

struct UnzoomedLine {
    polyline: PolyLine,
    width: Distance,
    color: Color,
}

struct UnzoomedCircle {
    center: Pt2D,
    radius: Distance,
    color: Color,
}

pub struct DrawUnzoomedShapesBuilder {
    lines: Vec<UnzoomedLine>,
    circles: Vec<UnzoomedCircle>,
}

impl DrawUnzoomedShapes {
    pub fn empty() -> Self {
        Self {
            lines: Vec::new(),
            circles: Vec::new(),
            per_zoom: Default::default(),
        }
    }

    pub fn builder() -> DrawUnzoomedShapesBuilder {
        DrawUnzoomedShapesBuilder {
            lines: Vec::new(),
            circles: Vec::new(),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let (zoom, idx) = discretize_zoom(g.canvas.cam_zoom);
        let value = &mut self.per_zoom.borrow_mut()[idx];
        if value.is_none() {
            // Never shrink past the original size -- always at least 1.0.
            // zoom ranges between [0.0, 1.0], and we want thicker shapes as zoom approaches 0.
            let max = 5.0;
            // So thickness ranges between [1.0, 5.0]
            let thickness = 1.0 + (max - 1.0) * (1.0 - zoom);

            let mut batch = GeomBatch::new();
            render_lines(&mut batch, &self.lines, thickness);
            render_circles(&mut batch, &self.circles, thickness);
            *value = Some(g.upload(batch));
        }
        g.redraw(value.as_ref().unwrap());
    }
}

impl DrawUnzoomedShapesBuilder {
    pub fn add_line(&mut self, polyline: PolyLine, width: Distance, color: Color) {
        self.lines.push(UnzoomedLine {
            polyline,
            width,
            color,
        });
    }

    pub fn add_circle(&mut self, center: Pt2D, radius: Distance, color: Color) {
        self.circles.push(UnzoomedCircle {
            center,
            radius,
            color,
        });
    }

    // TODO We might take EventCtx here to upload something to the GPU.
    pub fn build(self) -> DrawUnzoomedShapes {
        DrawUnzoomedShapes {
            lines: self.lines,
            circles: self.circles,
            per_zoom: Default::default(),
        }
    }
}

// Continuously changing road width as we zoom looks great, but it's terribly slow. We'd have to
// move line thickening into the shader to do it better. So recalculate with less granularity.
//
// Returns ([0.0, 1.0], [0, 10])
fn discretize_zoom(zoom: f64) -> (f64, usize) {
    if zoom >= 1.0 {
        return (1.0, 10);
    }
    let rounded = (zoom * 10.0).round();
    let idx = rounded as usize;
    (rounded / 10.0, idx)
}

fn render_lines(batch: &mut GeomBatch, lines: &[UnzoomedLine], thickness: f64) {
    for line in lines {
        batch.push(
            line.color,
            line.polyline.make_polygons(thickness * line.width),
        );
    }
}

fn render_circles(batch: &mut GeomBatch, circles: &[UnzoomedCircle], thickness: f64) {
    // TODO Here especially if we're drawing lots of circles with the same radius, generating the
    // shape once and translating it is much more efficient. UnzoomedAgents does this.
    for circle in circles {
        batch.push(
            circle.color,
            Circle::new(circle.center, thickness * circle.radius).to_polygon(),
        );
    }
}

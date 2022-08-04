use std::cell::RefCell;

use geom::{Circle, Distance, PolyLine, Pt2D};

use crate::{Color, Drawable, GeomBatch, GfxCtx};

/// Draw `Circles` and `PolyLines` in map-space that scale their size as the canvas is zoomed. The
/// goal is to appear with roughly constant screen-space size, but for the moment, this is
/// approximated by discretizing into 10 buckets. The scaling only happens when the canvas is
/// zoomed out less than a value of 1.0.
pub struct DrawUnzoomedShapes {
    shapes: Vec<Shape>,
    per_zoom: RefCell<[Option<Drawable>; 11]>,
}

enum Shape {
    Line {
        polyline: PolyLine,
        width: Distance,
        color: Color,
    },
    Circle {
        center: Pt2D,
        radius: Distance,
        color: Color,
    },
}

impl Shape {
    fn render(&self, batch: &mut GeomBatch, thickness: f64) {
        match self {
            Shape::Line {
                polyline,
                width,
                color,
            } => {
                batch.push(*color, polyline.make_polygons(thickness * *width));
            }
            Shape::Circle {
                center,
                radius,
                color,
            } => {
                // TODO Here especially if we're drawing lots of circles with the same radius,
                // generating the shape once and translating it is much more efficient.
                // UnzoomedAgents does this.
                batch.push(
                    *color,
                    Circle::new(*center, thickness * *radius).to_polygon(),
                );
            }
        }
    }
}

pub struct DrawUnzoomedShapesBuilder {
    shapes: Vec<Shape>,
}

impl DrawUnzoomedShapes {
    pub fn empty() -> Self {
        Self {
            shapes: Vec::new(),
            per_zoom: Default::default(),
        }
    }

    pub fn builder() -> DrawUnzoomedShapesBuilder {
        DrawUnzoomedShapesBuilder { shapes: Vec::new() }
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
            for shape in &self.shapes {
                shape.render(&mut batch, thickness);
            }
            *value = Some(g.upload(batch));
        }
        g.redraw(value.as_ref().unwrap());
    }
}

impl DrawUnzoomedShapesBuilder {
    pub fn add_line(&mut self, polyline: PolyLine, width: Distance, color: Color) {
        self.shapes.push(Shape::Line {
            polyline,
            width,
            color,
        });
    }

    pub fn add_circle(&mut self, center: Pt2D, radius: Distance, color: Color) {
        self.shapes.push(Shape::Circle {
            center,
            radius,
            color,
        });
    }

    pub fn build(self) -> DrawUnzoomedShapes {
        DrawUnzoomedShapes {
            shapes: self.shapes,
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

/// Draw custom objects that scale their size as the canvas is zoomed.
///
/// In all honesty I'm completely lost on the math here. By trial and error, I got something that
/// works reasonably for the one use case. Of course I'd love to properly understand how to do this
/// pattern, unify with the above, etc.
pub struct DrawCustomUnzoomedShapes {
    shapes: Vec<Box<dyn Fn(&mut GeomBatch, f64)>>,
    per_zoom: RefCell<PerZoom>,
}

pub struct DrawCustomUnzoomedShapesBuilder {
    shapes: Vec<Box<dyn Fn(&mut GeomBatch, f64)>>,
}

impl DrawCustomUnzoomedShapes {
    pub fn empty() -> Self {
        Self {
            shapes: Vec::new(),
            per_zoom: RefCell::new(PerZoom::new(1.0, 0.1)),
        }
    }

    pub fn builder() -> DrawCustomUnzoomedShapesBuilder {
        DrawCustomUnzoomedShapesBuilder { shapes: Vec::new() }
    }

    // If the zoom level is insufficient, return false
    pub fn maybe_draw(&self, g: &mut GfxCtx) -> bool {
        let mut per_zoom = self.per_zoom.borrow_mut();

        if g.canvas.cam_zoom >= per_zoom.min_zoom_for_detail {
            return false;
        }

        let (zoom, idx) = per_zoom.discretize_zoom(g.canvas.cam_zoom);
        let value = &mut per_zoom.draw_per_zoom[idx];
        if value.is_none() {
            let thickness = 1.0 / zoom;

            let mut batch = GeomBatch::new();
            for shape in &self.shapes {
                (shape)(&mut batch, thickness);
            }
            *value = Some(g.upload(batch));
        }
        g.redraw(value.as_ref().unwrap());

        true
    }
}

impl DrawCustomUnzoomedShapesBuilder {
    pub fn add_custom(&mut self, f: Box<dyn Fn(&mut GeomBatch, f64)>) {
        self.shapes.push(f);
    }

    pub fn build(self, per_zoom: PerZoom) -> DrawCustomUnzoomedShapes {
        DrawCustomUnzoomedShapes {
            shapes: self.shapes,
            per_zoom: RefCell::new(per_zoom),
        }
    }
}

// TODO There may be an off-by-one floating around here. Watch what this does at extremely low zoom
// levels near 0.
pub struct PerZoom {
    // TODO Maybe keep private and take the rendering callback here. Share more behavior with
    // DrawRoadLabels.
    pub draw_per_zoom: Vec<Option<Drawable>>,
    step_size: f64,
    min_zoom_for_detail: f64,
}

impl PerZoom {
    pub fn new(min_zoom_for_detail: f64, step_size: f64) -> Self {
        let num_buckets = (min_zoom_for_detail / step_size) as usize;
        Self {
            draw_per_zoom: std::iter::repeat_with(|| None).take(num_buckets).collect(),
            step_size,
            min_zoom_for_detail,
        }
    }

    // Takes the current canvas zoom, rounds it to the nearest step_size, and returns the index of
    // the bucket to fill out
    pub fn discretize_zoom(&self, zoom: f64) -> (f64, usize) {
        let bucket = (zoom / self.step_size).floor() as usize;
        // It's a bit weird to have the same zoom behavior for < 0.1 and 0.1 to 0.2, but unclear
        // what to do otherwise -- an effective zoom of 0 is broken
        let rounded = (bucket.max(1) as f64) * self.step_size;
        (rounded, bucket)
    }
}

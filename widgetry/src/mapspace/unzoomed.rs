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
    Custom(Box<dyn Fn(&mut GeomBatch, f64)>),
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
            Shape::Custom(f) => f(batch, thickness),
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

    /// Custom drawing code can add anything it wants to a batch, using a specified thickness in
    /// the [1.0, 5.0] range
    pub fn add_custom(&mut self, f: Box<dyn Fn(&mut GeomBatch, f64)>) {
        self.shapes.push(Shape::Custom(f));
    }

    // TODO We might take EventCtx here to upload something to the GPU.
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

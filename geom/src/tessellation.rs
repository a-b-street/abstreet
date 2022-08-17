use crate::{Angle, Bounds, Polygon, Pt2D};

// Deliberately not serializable
/// A tessellated polygon, ready for rendering.
#[derive(Clone)]
pub struct Tessellation {
    /// These points aren't in any meaningful order. It's not generally possible to reconstruct a
    /// `Polygon` from this.
    points: Vec<Pt2D>,
    /// Groups of three indices make up the triangles
    indices: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct Triangle {
    pub pt1: Pt2D,
    pub pt2: Pt2D,
    pub pt3: Pt2D,
}

impl From<Polygon> for Tessellation {
    fn from(polygon: Polygon) -> Self {
        Self {
            points: polygon.points,
            indices: polygon.indices,
        }
    }
}

impl Tessellation {
    pub fn new(points: Vec<Pt2D>, indices: Vec<usize>) -> Self {
        Tessellation {
            points,
            indices: downsize(indices),
        }
    }

    /// The `points` are not necessarily a `Ring`, which has strict requirements about no duplicate
    /// points. We can render various types of invalid polygon.
    pub fn from_ring(points: Vec<Pt2D>) -> Self {
        assert!(points.len() >= 3);

        let mut vertices = Vec::new();
        for pt in &points {
            vertices.push(pt.x());
            vertices.push(pt.y());
        }
        let indices = downsize(earcutr::earcut(&vertices, &Vec::new(), 2));

        Self { points, indices }
    }

    /// Returns (points, indices) for rendering
    pub fn consume(self) -> (Vec<Pt2D>, Vec<u16>) {
        (self.points, self.indices)
    }

    pub fn triangles(&self) -> Vec<Triangle> {
        let mut triangles: Vec<Triangle> = Vec::new();
        for slice in self.indices.chunks_exact(3) {
            triangles.push(Triangle {
                pt1: self.points[slice[0] as usize],
                pt2: self.points[slice[1] as usize],
                pt3: self.points[slice[2] as usize],
            });
        }
        triangles
    }

    pub fn get_bounds(&self) -> Bounds {
        Bounds::from(&self.points)
    }

    fn center(&self) -> Pt2D {
        self.get_bounds().center()
    }

    fn transform<F: Fn(&Pt2D) -> Pt2D>(&mut self, f: F) {
        for pt in &mut self.points {
            *pt = f(pt);
        }
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.transform(|pt| pt.offset(dx, dy));
    }

    pub fn scale(&mut self, factor: f64) {
        self.transform(|pt| Pt2D::new(pt.x() * factor, pt.y() * factor));
    }

    pub fn scale_xy(&mut self, x_factor: f64, y_factor: f64) {
        self.transform(|pt| Pt2D::new(pt.x() * x_factor, pt.y() * y_factor))
    }

    pub fn rotate(&mut self, angle: Angle) {
        self.rotate_around(angle, self.center())
    }

    pub fn rotate_around(&mut self, angle: Angle, pivot: Pt2D) {
        self.transform(|pt| {
            let origin_pt = Pt2D::new(pt.x() - pivot.x(), pt.y() - pivot.y());
            let (sin, cos) = angle.normalized_radians().sin_cos();
            Pt2D::new(
                pivot.x() + origin_pt.x() * cos - origin_pt.y() * sin,
                pivot.y() + origin_pt.y() * cos + origin_pt.x() * sin,
            )
        })
    }

    /// Equivalent to `self.scale(scale).translate(translate_x, translate_y).rotate_around(rotate,
    /// pivot)`, but modifies the polygon in-place and is faster.
    pub fn inplace_multi_transform(
        &mut self,
        scale: f64,
        translate_x: f64,
        translate_y: f64,
        rotate: Angle,
        pivot: Pt2D,
    ) {
        let (sin, cos) = rotate.normalized_radians().sin_cos();

        for pt in &mut self.points {
            // Scale
            let x = scale * pt.x();
            let y = scale * pt.y();
            // Translate
            let x = x + translate_x;
            let y = y + translate_y;
            // Rotate
            let origin_pt = Pt2D::new(x - pivot.x(), y - pivot.y());
            *pt = Pt2D::new(
                pivot.x() + origin_pt.x() * cos - origin_pt.y() * sin,
                pivot.y() + origin_pt.y() * cos + origin_pt.x() * sin,
            );
        }
    }
}

pub fn downsize(input: Vec<usize>) -> Vec<u16> {
    let mut output = Vec::new();
    for x in input {
        if let Ok(x) = u16::try_from(x) {
            output.push(x);
        } else {
            panic!("{} can't fit in u16, some polygon is too huge", x);
        }
    }
    output
}

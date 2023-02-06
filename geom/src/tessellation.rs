use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{Angle, Bounds, GPSBounds, Polygon, Pt2D};

// Only serializable for Polygons that precompute a tessellation
/// A tessellated polygon, ready for rendering.
#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub struct Tessellation {
    /// These points aren't in any meaningful order. It's not generally possible to reconstruct a
    /// `Polygon` from this.
    pub(crate) points: Vec<Pt2D>,
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
    fn from(mut polygon: Polygon) -> Self {
        if let Some(tessellation) = polygon.tessellation.take() {
            return tessellation;
        }

        let geojson_style: Vec<Vec<Vec<f64>>> = polygon
            .rings
            .iter()
            .map(|ring| {
                ring.points()
                    .iter()
                    .map(|pt| vec![pt.x(), pt.y()])
                    .collect()
            })
            .collect();
        let (vertices, holes, dims) = earcutr::flatten(&geojson_style);
        let indices = downsize(earcutr::earcut(&vertices, &holes, dims).unwrap());

        Self {
            points: vertices
                .chunks(2)
                .map(|pair| Pt2D::new(pair[0], pair[1]))
                .collect(),
            indices,
        }
    }
}

impl From<geo::Polygon> for Tessellation {
    fn from(poly: geo::Polygon) -> Self {
        // geo::Polygon -> geom::Polygon may fail, so we can't just do two hops. We can tessellate
        // something even if it has invalid Rings.
        let (exterior, mut interiors) = poly.into_inner();
        interiors.insert(0, exterior);

        let geojson_style: Vec<Vec<Vec<f64>>> = interiors
            .into_iter()
            .map(|ring| {
                ring.into_inner()
                    .into_iter()
                    .map(|pt| vec![pt.x, pt.y])
                    .collect()
            })
            .collect();
        let (vertices, holes, dims) = earcutr::flatten(&geojson_style);
        let indices = earcutr::earcut(&vertices, &holes, dims).unwrap();

        let points = vertices
            .chunks(2)
            .map(|pair| Pt2D::new(pair[0], pair[1]))
            .collect();

        Self::new(points, indices)
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
        let indices = downsize(earcutr::earcut(&vertices, &Vec::new(), 2).unwrap());

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

    pub fn center(&self) -> Pt2D {
        self.get_bounds().center()
    }

    pub(crate) fn transform<F: Fn(&Pt2D) -> Pt2D>(&mut self, f: F) {
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

    pub fn union(self, other: Self) -> Self {
        let mut points = self.points;
        let mut indices = self.indices;
        let offset = points.len() as u16;
        points.extend(other.points);
        for idx in other.indices {
            indices.push(offset + idx);
        }
        Self { points, indices }
    }

    pub fn union_all(mut list: Vec<Self>) -> Self {
        let mut result = list.pop().unwrap();
        for p in list {
            result = result.union(p);
        }
        result
    }

    /// Produces a GeoJSON multipolygon consisting of individual triangles. Optionally map the
    /// world-space points back to GPS.
    pub fn to_geojson(&self, gps: Option<&GPSBounds>) -> geojson::Geometry {
        let mut polygons = Vec::new();
        for triangle in self.triangles() {
            let raw_pts = vec![triangle.pt1, triangle.pt2, triangle.pt3, triangle.pt1];
            let mut pts = Vec::new();
            if let Some(gps) = gps {
                for pt in gps.convert_back(&raw_pts) {
                    pts.push(vec![pt.x(), pt.y()]);
                }
            } else {
                for pt in raw_pts {
                    pts.push(vec![pt.x(), pt.y()]);
                }
            }
            polygons.push(vec![pts]);
        }

        geojson::Geometry::new(geojson::Value::MultiPolygon(polygons))
    }

    // TODO This only makes sense for something vaguely Ring-like
    fn to_geo(&self) -> geo::Polygon {
        let exterior = crate::conversions::pts_to_line_string(&self.points);
        geo::Polygon::new(exterior, Vec::new())
    }

    // TODO After making to_outline return a real Polygon, get rid of this
    pub fn difference(&self, other: &Tessellation) -> Result<Vec<Polygon>> {
        use geo::BooleanOps;

        // TODO Remove after https://github.com/georust/geo/issues/913
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::polygon::from_multi(self.to_geo().difference(&other.to_geo()))
        })) {
            Ok(result) => result,
            Err(err) => {
                println!("BooleanOps crashed: {err:?}");
                bail!("BooleanOps crashed: {err:?}");
            }
        }
    }
}

fn downsize(input: Vec<usize>) -> Vec<u16> {
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

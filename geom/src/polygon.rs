use crate::{Bounds, Distance, HashablePt2D, Pt2D};
use geo_booleanop::boolean::BooleanOp;
use geo_offset::Offset;
use serde_derive::{Deserialize, Serialize};
use std::fmt;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Polygon {
    points: Vec<Pt2D>,
    // Groups of three indices make up the triangles
    // TODO u32 better for later, but then we can't index stuff!
    indices: Vec<usize>,
}

// TODO The triangulation is a bit of a mess. Everything except for Polygon::new comes from
// https://github.com/lionfish0/earclip/blob/master/earclip/__init__.py.

impl Polygon {
    // TODO Should the first and last points match or not?
    // Adapted from https://crates.io/crates/polygon2; couldn't use the crate directly because it
    // depends on nightly.
    pub fn new(orig_pts: &Vec<Pt2D>) -> Polygon {
        assert!(orig_pts.len() >= 3);

        let pts = if is_clockwise_polygon(orig_pts) {
            let mut new_pts = orig_pts.clone();
            new_pts.reverse();
            new_pts
        } else {
            orig_pts.clone()
        };

        let mut indices: Vec<usize> = Vec::new();
        let mut avl = Vec::with_capacity(pts.len());
        for i in 0..pts.len() {
            avl.push(i);
        }

        let mut i = 0;
        let mut al = pts.len();
        while al > 3 {
            let i0 = avl[i % al];
            let i1 = avl[(i + 1) % al];
            let i2 = avl[(i + 2) % al];

            let tri = Triangle::new(pts[i0], pts[i1], pts[i2]);
            let mut ear_found = false;
            if tri.is_convex() {
                ear_found = true;

                for vi in avl.iter().take(al) {
                    if *vi != i0 && *vi != i1 && *vi != i2 && tri.contains_pt(pts[*vi]) {
                        ear_found = false;
                        break;
                    }
                }
            }

            if ear_found {
                indices.push(i0);
                indices.push(i1);
                indices.push(i2);
                avl.remove((i + 1) % al);
                al -= 1;
                i = 0;
            } else if i > 3 * al {
                break;
            } else {
                i += 1;
            }
        }

        indices.push(avl[0]);
        indices.push(avl[1]);
        indices.push(avl[2]);

        Polygon {
            points: pts,
            indices,
        }
    }

    pub fn precomputed(points: Vec<Pt2D>, indices: Vec<usize>) -> Polygon {
        assert!(indices.len() % 3 == 0);
        Polygon { points, indices }
    }

    pub fn from_triangle(tri: &Triangle) -> Polygon {
        Polygon {
            points: vec![tri.pt1, tri.pt2, tri.pt3],
            indices: vec![0, 1, 2],
        }
    }

    pub fn triangles(&self) -> Vec<Triangle> {
        let mut triangles: Vec<Triangle> = Vec::new();
        for slice in self.indices.chunks_exact(3) {
            triangles.push(Triangle::new(
                self.points[slice[0]],
                self.points[slice[1]],
                self.points[slice[2]],
            ));
        }
        triangles
    }

    pub fn raw_for_rendering(&self) -> (&Vec<Pt2D>, &Vec<usize>) {
        (&self.points, &self.indices)
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.triangles().into_iter().any(|tri| tri.contains_pt(pt))
    }

    pub fn get_bounds(&self) -> Bounds {
        Bounds::from(&self.points)
    }

    pub fn translate(&self, dx: Distance, dy: Distance) -> Polygon {
        Polygon {
            points: self.points.iter().map(|pt| pt.offset(dx, dy)).collect(),
            indices: self.indices.clone(),
        }
    }

    // The order of these points depends on the constructor! The first and last point may or may
    // not match. Polygons constructed from PolyLines will have a very weird order.
    pub fn points(&self) -> &Vec<Pt2D> {
        &self.points
    }

    pub fn center(&self) -> Pt2D {
        // TODO dedupe just out of fear of the first/last point being repeated
        let mut pts: Vec<HashablePt2D> = self.points.iter().map(|pt| pt.to_hashable()).collect();
        pts.sort();
        pts.dedup();
        Pt2D::center(&pts.iter().map(|pt| pt.to_pt2d()).collect())
    }

    pub fn rectangle(center: Pt2D, width: Distance, height: Distance) -> Polygon {
        Polygon::rectangle_topleft(center.offset(-width / 2.0, -height / 2.0), width, height)
    }

    pub fn rectangle_topleft(top_left: Pt2D, width: Distance, height: Distance) -> Polygon {
        Polygon {
            points: vec![
                top_left,
                top_left.offset(width, Distance::ZERO),
                top_left.offset(width, height),
                top_left.offset(Distance::ZERO, height),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    pub fn rectangle_two_corners(pt1: Pt2D, pt2: Pt2D) -> Option<Polygon> {
        if Pt2D::new(pt1.x(), 0.0).epsilon_eq(Pt2D::new(pt2.x(), 0.0))
            || Pt2D::new(0.0, pt1.y()).epsilon_eq(Pt2D::new(0.0, pt2.y()))
        {
            return None;
        }

        let (x1, width) = if pt1.x() < pt2.x() {
            (pt1.x(), Distance::meters(pt2.x() - pt1.x()))
        } else {
            (pt2.x(), Distance::meters(pt1.x() - pt2.x()))
        };
        let (y1, height) = if pt1.y() < pt2.y() {
            (pt1.y(), Distance::meters(pt2.y() - pt1.y()))
        } else {
            (pt2.y(), Distance::meters(pt1.y() - pt2.y()))
        };
        Some(Polygon::rectangle_topleft(Pt2D::new(x1, y1), width, height))
    }

    pub fn union(self, other: Polygon) -> Polygon {
        let mut points = self.points;
        let mut indices = self.indices;
        let offset = points.len();
        points.extend(other.points);
        for idx in other.indices {
            indices.push(offset + idx);
        }
        Polygon::precomputed(points, indices)
    }

    pub fn intersection(&self, other: &Polygon) -> Vec<Polygon> {
        from_multi(to_geo(self.points()).intersection(&to_geo(other.points())))
    }

    pub fn polylabel(&self) -> Pt2D {
        let pt = polylabel::polylabel(&to_geo(&self.points()), &1.0);
        Pt2D::new(pt.x(), pt.y())
    }

    pub fn shrink(&self, distance: f64) -> Vec<Polygon> {
        from_multi(to_geo(self.points()).offset(distance).unwrap())
    }
}

impl fmt::Display for Polygon {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "Polygon with {} points and {} indices",
            self.points.len(),
            self.indices.len()
        )?;
        for (idx, pt) in self.points.iter().enumerate() {
            writeln!(f, "  {}: {}", idx, pt)?;
        }
        write!(f, "Indices: [")?;
        for slice in self.indices.chunks_exact(3) {
            write!(f, "({}, {}, {}), ", slice[0], slice[1], slice[2])?;
        }
        writeln!(f, "]")
    }
}

#[derive(Clone, Debug)]
pub struct Triangle {
    pub pt1: Pt2D,
    pub pt2: Pt2D,
    pub pt3: Pt2D,
}

impl Triangle {
    pub(crate) fn new(pt1: Pt2D, pt2: Pt2D, pt3: Pt2D) -> Triangle {
        Triangle { pt1, pt2, pt3 }
    }

    fn is_convex(&self) -> bool {
        let x1 = self.pt1.x();
        let y1 = self.pt1.y();
        let x2 = self.pt2.x();
        let y2 = self.pt2.y();
        let x3 = self.pt3.x();
        let y3 = self.pt3.y();

        let cross_product = (x2 - x1) * (y3 - y1) - (y2 - y1) * (x3 - x1);
        cross_product >= 0.0
    }

    fn contains_pt(&self, pt: Pt2D) -> bool {
        let x1 = self.pt1.x();
        let y1 = self.pt1.y();
        let x2 = self.pt2.x();
        let y2 = self.pt2.y();
        let x3 = self.pt3.x();
        let y3 = self.pt3.y();
        let px = pt.x();
        let py = pt.y();

        // Barycentric coefficients for pt
        // Use epsilon to deal with small denominators
        let epsilon = 0.000_000_1;
        let l0 = ((y2 - y3) * (px - x3) + (x3 - x2) * (py - y3))
            / (((y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3)) + epsilon);
        let l1 = ((y3 - y1) * (px - x3) + (x1 - x3) * (py - y3))
            / (((y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3)) + epsilon);
        let l2 = 1.0 - l0 - l1;

        for x in &[l0, l1, l2] {
            if *x >= 1.0 || *x <= 0.0 {
                return false;
            }
        }
        true
    }
}

fn is_clockwise_polygon(pts: &Vec<Pt2D>) -> bool {
    // Initialize with the last element
    let mut sum = (pts[0].x() - pts.last().unwrap().x()) * (pts[0].y() + pts.last().unwrap().y());
    for i in 0..pts.len() - 1 {
        sum += (pts[i + 1].x() - pts[i].x()) * (pts[i + 1].y() + pts[i].y());
    }
    sum > 0.0
}

fn to_geo(pts: &Vec<Pt2D>) -> geo::Polygon<f64> {
    geo::Polygon::new(
        geo::LineString::from(
            pts.iter()
                .map(|pt| geo::Point::new(pt.x(), pt.y()))
                .collect::<Vec<_>>(),
        ),
        Vec::new(),
    )
}

fn from_multi(multi: geo::MultiPolygon<f64>) -> Vec<Polygon> {
    multi
        .into_iter()
        .map(|poly| {
            Polygon::new(
                &poly
                    .into_inner()
                    .0
                    .into_points()
                    .into_iter()
                    .map(|pt| Pt2D::new(pt.x(), pt.y()))
                    .collect(),
            )
        })
        .collect()
}

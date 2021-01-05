use std::convert::TryFrom;
use std::fmt;

use anyhow::Result;
use geo::algorithm::area::Area;
use geo::algorithm::concave_hull::ConcaveHull;
use geo::algorithm::convex_hull::ConvexHull;
use geo_booleanop::boolean::BooleanOp;
use serde::{Deserialize, Serialize};

use crate::{Angle, Bounds, Distance, GPSBounds, HashablePt2D, PolyLine, Pt2D, Ring};

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub struct Polygon {
    points: Vec<Pt2D>,
    /// Groups of three indices make up the triangles
    indices: Vec<u16>,

    /// If the polygon has holes, explicitly store all the rings (the one outer and all of the
    /// inner) so they can later be used to generate outlines and such. If the polygon has no
    /// holes, then this will just be None, since the points form a ring.
    rings: Option<Vec<Ring>>,
}

impl Polygon {
    // TODO Last result when we've got something that isn't a valid Ring, but want to draw it
    // anyway. Fix the root cause of those cases instead.
    pub fn buggy_new(orig_pts: Vec<Pt2D>) -> Polygon {
        assert!(orig_pts.len() >= 3);

        let mut vertices = Vec::new();
        for pt in &orig_pts {
            vertices.push(pt.x());
            vertices.push(pt.y());
        }
        let indices = downsize(earcutr::earcut(&vertices, &Vec::new(), 2));

        Polygon {
            points: orig_pts.clone(),
            indices,
            rings: None,
        }
    }

    pub fn with_holes(outer: Ring, mut inner: Vec<Ring>) -> Polygon {
        inner.insert(0, outer);
        let geojson_style: Vec<Vec<Vec<f64>>> = inner
            .iter()
            .map(|ring| {
                ring.points()
                    .into_iter()
                    .map(|pt| vec![pt.x(), pt.y()])
                    .collect()
            })
            .collect();
        let (vertices, holes, dims) = earcutr::flatten(&geojson_style);
        let indices = downsize(earcutr::earcut(&vertices, &holes, dims));

        Polygon {
            points: vertices
                .chunks(2)
                .map(|pair| Pt2D::new(pair[0], pair[1]))
                .collect(),
            indices,
            rings: if inner.len() == 1 { None } else { Some(inner) },
        }
    }

    // TODO Doesn't remember rings yet
    pub fn from_geojson(raw: &Vec<Vec<Vec<f64>>>) -> Polygon {
        let (vertices, holes, dims) = earcutr::flatten(raw);
        let indices = downsize(earcutr::earcut(&vertices, &holes, dims));
        Polygon {
            points: vertices
                .chunks(2)
                .map(|pair| Pt2D::new(pair[0], pair[1]))
                .collect(),
            indices,
            rings: None,
        }
    }

    // TODO No guarantee points forms a ring. In fact, the main caller is from SVG->lyon parsing,
    // and it's NOT true there yet.
    pub fn precomputed(points: Vec<Pt2D>, indices: Vec<usize>) -> Polygon {
        assert!(indices.len() % 3 == 0);
        Polygon {
            points,
            indices: downsize(indices),
            rings: None,
        }
    }

    pub fn from_triangle(tri: &Triangle) -> Polygon {
        Polygon {
            points: vec![tri.pt1, tri.pt2, tri.pt3, tri.pt1],
            indices: vec![0, 1, 2],
            rings: None,
        }
    }

    pub fn triangles(&self) -> Vec<Triangle> {
        let mut triangles: Vec<Triangle> = Vec::new();
        for slice in self.indices.chunks_exact(3) {
            triangles.push(Triangle::new(
                self.points[slice[0] as usize],
                self.points[slice[1] as usize],
                self.points[slice[2] as usize],
            ));
        }
        triangles
    }

    pub fn raw_for_rendering(&self) -> (&Vec<Pt2D>, &Vec<u16>) {
        (&self.points, &self.indices)
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.triangles().into_iter().any(|tri| tri.contains_pt(pt))
    }

    pub fn get_bounds(&self) -> Bounds {
        Bounds::from(&self.points)
    }

    fn transform<F: Fn(&Pt2D) -> Pt2D>(&self, f: F) -> Polygon {
        Polygon {
            points: self.points.iter().map(&f).collect(),
            indices: self.indices.clone(),
            rings: self.rings.as_ref().map(|rings| {
                rings
                    .iter()
                    // When scaling, rings may collapse entirely; just give up on preserving in
                    // that case.
                    .filter_map(|ring| Ring::new(ring.points().iter().map(&f).collect()).ok())
                    .collect()
            }),
        }
    }

    pub fn translate(&self, dx: f64, dy: f64) -> Polygon {
        self.transform(|pt| pt.offset(dx, dy))
    }

    pub fn scale(&self, factor: f64) -> Polygon {
        self.transform(|pt| Pt2D::new(pt.x() * factor, pt.y() * factor))
    }

    pub fn rotate(&self, angle: Angle) -> Polygon {
        self.rotate_around(angle, self.center())
    }

    pub fn rotate_around(&self, angle: Angle, pivot: Pt2D) -> Polygon {
        self.transform(|pt| {
            let origin_pt = Pt2D::new(pt.x() - pivot.x(), pt.y() - pivot.y());
            let (sin, cos) = angle.normalized_radians().sin_cos();
            Pt2D::new(
                pivot.x() + origin_pt.x() * cos - origin_pt.y() * sin,
                pivot.y() + origin_pt.y() * cos + origin_pt.x() * sin,
            )
        })
    }

    /// The order of these points depends on the constructor! The first and last point may or may
    /// not match. Polygons constructed from PolyLines will have a very weird order.
    // TODO rename outer_points to be clear
    pub fn points(&self) -> &Vec<Pt2D> {
        if let Some(ref rings) = self.rings {
            rings[0].points()
        } else {
            &self.points
        }
    }
    pub fn into_points(mut self) -> Vec<Pt2D> {
        if let Some(mut rings) = self.rings.take() {
            rings.remove(0).into_points()
        } else {
            self.points
        }
    }
    pub fn into_ring(self) -> Ring {
        Ring::must_new(self.into_points())
    }

    pub fn center(&self) -> Pt2D {
        // TODO dedupe just out of fear of the first/last point being repeated
        let mut pts: Vec<HashablePt2D> = self.points.iter().map(|pt| pt.to_hashable()).collect();
        pts.sort();
        pts.dedup();
        Pt2D::center(&pts.iter().map(|pt| pt.to_pt2d()).collect())
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    pub fn rectangle(width: f64, height: f64) -> Polygon {
        Polygon {
            points: vec![
                Pt2D::new(0.0, 0.0),
                Pt2D::new(width, 0.0),
                Pt2D::new(width, height),
                Pt2D::new(0.0, height),
                Pt2D::new(0.0, 0.0),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            rings: None,
        }
    }

    pub fn rectangle_centered(center: Pt2D, width: Distance, height: Distance) -> Polygon {
        Polygon::rectangle(width.inner_meters(), height.inner_meters()).translate(
            center.x() - width.inner_meters() / 2.0,
            center.y() - height.inner_meters() / 2.0,
        )
    }

    pub fn rectangle_two_corners(pt1: Pt2D, pt2: Pt2D) -> Option<Polygon> {
        if Pt2D::new(pt1.x(), 0.0) == Pt2D::new(pt2.x(), 0.0)
            || Pt2D::new(0.0, pt1.y()) == Pt2D::new(0.0, pt2.y())
        {
            return None;
        }

        let (x1, width) = if pt1.x() < pt2.x() {
            (pt1.x(), pt2.x() - pt1.x())
        } else {
            (pt2.x(), pt1.x() - pt2.x())
        };
        let (y1, height) = if pt1.y() < pt2.y() {
            (pt1.y(), pt2.y() - pt1.y())
        } else {
            (pt2.y(), pt1.y() - pt2.y())
        };
        Some(Polygon::rectangle(width, height).translate(x1, y1))
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    /// If radius is None, be as round as possible. Fails if the radius is too big.
    pub fn maybe_rounded_rectangle(w: f64, h: f64, r: Option<f64>) -> Option<Polygon> {
        let r = r.unwrap_or_else(|| w.min(h) / 2.0);
        if 2.0 * r > w || 2.0 * r > h {
            return None;
        }

        let mut pts = vec![];

        const RESOLUTION: usize = 5;
        let mut arc = |center: Pt2D, angle1_degs: f64, angle2_degs: f64| {
            for i in 0..=RESOLUTION {
                let angle = Angle::degrees(
                    angle1_degs + (angle2_degs - angle1_degs) * ((i as f64) / (RESOLUTION as f64)),
                );
                pts.push(center.project_away(Distance::meters(r), angle.invert_y()));
            }
        };

        // Top-left corner
        arc(Pt2D::new(r, r), 180.0, 90.0);
        // Top-right
        arc(Pt2D::new(w - r, r), 90.0, 0.0);
        // Bottom-right
        arc(Pt2D::new(w - r, h - r), 360.0, 270.0);
        // Bottom-left
        arc(Pt2D::new(r, h - r), 270.0, 180.0);
        // Close it off
        pts.push(Pt2D::new(0.0, r));

        // If the radius was maximized, then some of the edges will be zero length.
        pts.dedup();

        Some(Ring::must_new(pts).to_polygon())
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    /// If radius is None, be as round as possible
    pub fn rounded_rectangle(w: f64, h: f64, r: Option<f64>) -> Polygon {
        Polygon::maybe_rounded_rectangle(w, h, r).unwrap()
    }

    // TODO Result won't be a nice Ring
    pub fn union(self, other: Polygon) -> Polygon {
        let mut points = self.points;
        let mut indices = self.indices;
        let offset = points.len() as u16;
        points.extend(other.points);
        for idx in other.indices {
            indices.push(offset + idx);
        }
        Polygon {
            points,
            indices,
            rings: None,
        }
    }

    pub fn union_all(mut list: Vec<Polygon>) -> Polygon {
        let mut result = list.pop().unwrap();
        for p in list {
            result = result.union(p);
        }
        result
    }

    // TODO Result won't be a nice Ring
    pub fn intersection(&self, other: &Polygon) -> Vec<Polygon> {
        from_multi(to_geo(self.points()).intersection(&to_geo(other.points())))
    }

    pub fn convex_hull(list: Vec<Polygon>) -> Polygon {
        let mp: geo::MultiPolygon<f64> = list.into_iter().map(|p| to_geo(p.points())).collect();
        mp.convex_hull().into()
    }

    pub fn concave_hull(list: Vec<Polygon>, concavity: f64) -> Polygon {
        let mp: geo::MultiPolygon<f64> = list.into_iter().map(|p| to_geo(p.points())).collect();
        mp.concave_hull(concavity).into()
    }

    pub fn polylabel(&self) -> Pt2D {
        let pt = polylabel::polylabel(&to_geo(&self.points()), &1.0).unwrap();
        Pt2D::new(pt.x(), pt.y())
    }

    /// Creates the outline around the polygon, with the thickness half straddling the polygon and
    /// half of it just outside. Only works for polygons that're formed from rings. Those made from
    /// PolyLines won't work, for example.
    pub fn to_outline(&self, thickness: Distance) -> Result<Polygon> {
        if let Some(ref rings) = self.rings {
            Ok(Polygon::union_all(
                rings.iter().map(|r| r.to_outline(thickness)).collect(),
            ))
        } else {
            Ring::new(self.points.clone()).map(|r| r.to_outline(thickness))
        }
    }

    /// Remove the internal rings used for to_outline. This is fine to do if the polygon is being
    /// added to some larger piece of geometry that won't need an outline.
    pub fn strip_rings(&self) -> Polygon {
        let mut p = self.clone();
        p.rings = None;
        p
    }

    /// Usually m^2, unless the polygon is in screen-space
    pub fn area(&self) -> f64 {
        // Polygon orientation messes this up sometimes
        to_geo(&self.points()).unsigned_area()
    }

    /// Doesn't handle multiple crossings in and out.
    pub fn clip_polyline(&self, input: &PolyLine) -> Option<Vec<Pt2D>> {
        let ring = Ring::must_new(self.points.clone());
        let hits = ring.all_intersections(input);

        if hits.len() == 0 {
            // All the points must be inside, or none
            if self.contains_pt(input.first_pt()) {
                Some(input.points().clone())
            } else {
                None
            }
        } else if hits.len() == 1 {
            // Which end?
            if self.contains_pt(input.first_pt()) {
                input
                    .get_slice_ending_at(hits[0])
                    .map(|pl| pl.into_points())
            } else {
                input
                    .get_slice_starting_at(hits[0])
                    .map(|pl| pl.into_points())
            }
        } else if hits.len() == 2 {
            Some(input.trim_to_endpts(hits[0], hits[1]).into_points())
        } else {
            // TODO Not handled
            None
        }
    }

    // TODO Only handles a few cases
    pub fn clip_ring(&self, input: &Ring) -> Option<Vec<Pt2D>> {
        let ring = Ring::must_new(self.points.clone());
        let hits = ring.all_intersections(&PolyLine::unchecked_new(input.clone().into_points()));

        if hits.len() == 0 {
            // If the first point is inside, then all must be
            if self.contains_pt(input.points()[0]) {
                return Some(input.points().clone());
            }
        } else if hits.len() == 2 {
            let (pl1, pl2) = input.get_both_slices_btwn(hits[0], hits[1])?;

            // One of these should be partly outside the polygon. The endpoints won't be in the
            // polygon itself, but they'll be on the ring.
            if pl1
                .points()
                .iter()
                .all(|pt| self.contains_pt(*pt) || ring.contains_pt(*pt))
            {
                return Some(pl1.into_points());
            }
            if pl2
                .points()
                .iter()
                .all(|pt| self.contains_pt(*pt) || ring.contains_pt(*pt))
            {
                return Some(pl2.into_points());
            }
            // Huh?
        }

        None
    }

    /// If the polygon is just a single outer ring, produces a GeoJSON polygon. Otherwise, produces
    /// a GeoJSON multipolygon consisting of individual triangles. Optionally map the world-space
    /// points back to GPS.
    pub fn to_geojson(&self, gps: Option<&GPSBounds>) -> geojson::Geometry {
        if let Ok(ring) = Ring::new(self.points.clone()) {
            return ring.to_geojson(gps);
        }

        let mut polygons = Vec::new();
        for triangle in self.triangles() {
            let raw_pts = vec![triangle.pt1, triangle.pt2, triangle.pt3, triangle.pt1];
            let mut pts = Vec::new();
            if let Some(ref gps) = gps {
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

impl From<geo::Polygon<f64>> for Polygon {
    fn from(poly: geo::Polygon<f64>) -> Self {
        let (exterior, interiors) = poly.into_inner();
        Polygon::with_holes(
            Ring::from(exterior),
            interiors.into_iter().map(Ring::from).collect(),
        )
    }
}

impl From<Polygon> for geo::Polygon<f64> {
    fn from(poly: Polygon) -> Self {
        if let Some(mut rings) = poly.rings {
            let exterior = rings.pop().expect("expected poly.rings[0] to be exterior");
            let interiors: Vec<geo::LineString<f64>> =
                rings.into_iter().map(geo::LineString::from).collect();
            Self::new(exterior.into(), interiors)
        } else {
            let exterior_coords = poly
                .points
                .into_iter()
                .map(geo::Coordinate::from)
                .collect::<Vec<_>>();
            let exterior = geo::LineString(exterior_coords);
            Self::new(exterior, Vec::new())
        }
    }
}

fn from_multi(multi: geo::MultiPolygon<f64>) -> Vec<Polygon> {
    // TODO This should just call Polygon::from, but while importing maps, it seems like
    // intersection() is hitting non-Ring cases that crash. So keep using buggy_new for now.
    multi
        .into_iter()
        .map(|p| {
            let pts = p
                .into_inner()
                .0
                .into_points()
                .into_iter()
                .map(|pt| Pt2D::new(pt.x(), pt.y()))
                .collect();
            Polygon::buggy_new(pts)
        })
        .collect()
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

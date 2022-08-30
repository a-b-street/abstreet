use std::fmt;

use anyhow::Result;
use geo::{Area, BooleanOps, Contains, ConvexHull, Intersects, SimplifyVWPreserve};
use serde::{Deserialize, Serialize};

use abstutil::Tags;

use crate::{
    Angle, Bounds, CornerRadii, Distance, GPSBounds, HashablePt2D, LonLat, PolyLine, Pt2D, Ring,
    Tessellation, Triangle,
};

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub struct Polygon {
    pub(crate) points: Vec<Pt2D>,
    /// Groups of three indices make up the triangles
    pub(crate) indices: Vec<u16>,

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
        let indices = crate::tessellation::downsize(earcutr::earcut(&vertices, &Vec::new(), 2));

        Polygon {
            points: orig_pts,
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
                    .iter()
                    .map(|pt| vec![pt.x(), pt.y()])
                    .collect()
            })
            .collect();
        let (vertices, holes, dims) = earcutr::flatten(&geojson_style);
        let indices = crate::tessellation::downsize(earcutr::earcut(&vertices, &holes, dims));

        Polygon {
            points: vertices
                .chunks(2)
                .map(|pair| Pt2D::new(pair[0], pair[1]))
                .collect(),
            indices,
            rings: if inner.len() == 1 { None } else { Some(inner) },
        }
    }

    pub fn from_rings(mut rings: Vec<Ring>) -> Polygon {
        assert!(!rings.is_empty());
        let outer = rings.remove(0);
        Polygon::with_holes(outer, rings)
    }

    pub fn from_geojson(raw: &[Vec<Vec<f64>>]) -> Result<Polygon> {
        let mut rings = Vec::new();
        for pts in raw {
            let transformed: Vec<Pt2D> =
                pts.iter().map(|pair| Pt2D::new(pair[0], pair[1])).collect();
            rings.push(Ring::new(transformed)?);
        }
        Ok(Polygon::from_rings(rings))
    }

    // TODO No guarantee points forms a ring. In fact, the main caller is from SVG->lyon parsing,
    // and it's NOT true there yet.
    pub fn precomputed(points: Vec<Pt2D>, indices: Vec<usize>) -> Polygon {
        assert!(indices.len() % 3 == 0);
        Polygon {
            points,
            indices: crate::tessellation::downsize(indices),
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
        Tessellation::from(self.clone()).triangles()
    }

    /// Does this polygon contain the point in its interior?
    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.to_geo().contains(&geo::Point::from(pt))
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

    pub fn scale_xy(&self, x_factor: f64, y_factor: f64) -> Polygon {
        self.transform(|pt| Pt2D::new(pt.x() * x_factor, pt.y() * y_factor))
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

    pub fn centered_on(&self, center: Pt2D) -> Polygon {
        let bounds = self.get_bounds();
        let dx = center.x() - bounds.width() / 2.0;
        let dy = center.y() - bounds.height() / 2.0;
        self.translate(dx, dy)
    }

    /// Equivalent to `self.strip_rings().scale(scale).translate(translate_x, translate_y).rotate_around(rotate,
    /// pivot)`, but modifies the polygon in-place and is faster.
    pub fn inplace_multi_transform(
        &mut self,
        scale: f64,
        translate_x: f64,
        translate_y: f64,
        rotate: Angle,
        pivot: Pt2D,
    ) {
        // Give up on preserving rings; they might collapse down during transformation. It's a
        // waste of performance anyway; this method is useful mainly for tesselations.
        self.rings = None;

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

    /// Get the outer ring of this polygon. This should usually succeed.
    pub fn get_outer_ring(&self) -> Option<Ring> {
        if let Some(ref rings) = self.rings {
            Some(rings[0].clone())
        } else {
            Ring::new(self.points.clone()).ok()
        }
    }

    pub fn center(&self) -> Pt2D {
        // TODO dedupe just out of fear of the first/last point being repeated
        let mut pts: Vec<HashablePt2D> = self.points.iter().map(|pt| pt.to_hashable()).collect();
        pts.sort();
        pts.dedup();
        Pt2D::center(&pts.iter().map(|pt| pt.to_pt2d()).collect::<Vec<_>>())
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
    pub fn maybe_rounded_rectangle<R: Into<CornerRadii>>(w: f64, h: f64, r: R) -> Option<Polygon> {
        let r = r.into();
        let max_r = r
            .top_left
            .max(r.top_right)
            .max(r.bottom_right)
            .max(r.bottom_left);
        if 2.0 * max_r > w || 2.0 * max_r > h {
            return None;
        }

        let mut pts = vec![];

        const RESOLUTION: usize = 5;
        let mut arc = |r: f64, center: Pt2D, angle1_degs: f64, angle2_degs: f64| {
            for i in 0..=RESOLUTION {
                let angle = Angle::degrees(
                    angle1_degs + (angle2_degs - angle1_degs) * ((i as f64) / (RESOLUTION as f64)),
                );
                pts.push(center.project_away(Distance::meters(r), angle.invert_y()));
            }
        };

        arc(r.top_left, Pt2D::new(r.top_left, r.top_left), 180.0, 90.0);
        arc(
            r.top_right,
            Pt2D::new(w - r.top_right, r.top_right),
            90.0,
            0.0,
        );
        arc(
            r.bottom_right,
            Pt2D::new(w - r.bottom_right, h - r.bottom_right),
            360.0,
            270.0,
        );
        arc(
            r.bottom_left,
            Pt2D::new(r.bottom_left, h - r.bottom_left),
            270.0,
            180.0,
        );
        // Close it off
        pts.push(Pt2D::new(0.0, r.top_left));

        // If the radius was maximized, then some of the edges will be zero length.
        pts.dedup();

        Some(Ring::must_new(pts).into_polygon())
    }

    /// A rectangle, two sides of which are fully rounded half-circles.
    pub fn pill(w: f64, h: f64) -> Polygon {
        let r = w.min(h) / 2.0;
        Polygon::maybe_rounded_rectangle(w, h, r).unwrap()
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    /// If it's not possible to apply the specified radius, fallback to a regular rectangle.
    pub fn rounded_rectangle<R: Into<CornerRadii>>(w: f64, h: f64, r: R) -> Polygon {
        Polygon::maybe_rounded_rectangle(w, h, r).unwrap_or_else(|| Polygon::rectangle(w, h))
    }

    /// Union all of the polygons into one geo::MultiPolygon
    pub fn union_all_into_multipolygon(mut list: Vec<Polygon>) -> geo::MultiPolygon {
        // TODO Not sure why this happened, or if this is really valid to construct...
        if list.is_empty() {
            return geo::MultiPolygon(Vec::new());
        }

        let mut result = geo::MultiPolygon(vec![list.pop().unwrap().into()]);
        for p in list {
            result = result.union(&p.into());
        }
        result
    }

    pub fn intersection(&self, other: &Polygon) -> Vec<Polygon> {
        from_multi(self.to_geo().intersection(&other.to_geo()))
    }

    pub fn difference(&self, other: &Polygon) -> Vec<Polygon> {
        from_multi(self.to_geo().difference(&other.to_geo()))
    }

    pub fn convex_hull(list: Vec<Polygon>) -> Polygon {
        let mp: geo::MultiPolygon = list.into_iter().map(|p| p.to_geo()).collect();
        mp.convex_hull().into()
    }

    pub fn concave_hull(points: Vec<Pt2D>, concavity: u32) -> Polygon {
        use geo::k_nearest_concave_hull::KNearestConcaveHull;
        let points: Vec<geo::Point> = points.iter().map(|p| geo::Point::from(*p)).collect();
        points.k_nearest_concave_hull(concavity).into()
    }

    /// Find the "pole of inaccessibility" -- the most distant internal point from the polygon
    /// outline
    pub fn polylabel(&self) -> Pt2D {
        let pt = polylabel::polylabel(&self.to_geo(), &1.0).unwrap();
        Pt2D::new(pt.x(), pt.y())
    }

    /// Do two polygons intersect at all?
    pub fn intersects(&self, other: &Polygon) -> bool {
        self.to_geo().intersects(&other.to_geo())
    }

    /// Does this polygon intersect a polyline?
    pub fn intersects_polyline(&self, pl: &PolyLine) -> bool {
        self.to_geo().intersects(&pl.to_geo())
    }

    /// Creates the outline around the polygon (both the exterior and holes), with the thickness
    /// half straddling the polygon and half of it just outside. Only works for polygons that're
    /// formed from rings.
    ///
    /// Returns a `Tessellation` that may union together the outline from the exterior and multiple
    /// holes. Callers that need a `Polygon` must call `to_outline` on the individual `Rings`.
    pub fn to_outline(&self, thickness: Distance) -> Result<Tessellation> {
        if let Some(ref rings) = self.rings {
            Ok(Tessellation::union_all(
                rings
                    .iter()
                    .map(|r| Tessellation::from(r.to_outline(thickness)))
                    .collect(),
            ))
        } else {
            Ring::new(self.points.clone()).map(|r| Tessellation::from(r.to_outline(thickness)))
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
        // Don't use signed_area, since we may work with polygons that have different orientations
        self.to_geo().unsigned_area()
    }

    /// Doesn't handle multiple crossings in and out.
    pub fn clip_polyline(&self, input: &PolyLine) -> Option<Vec<Pt2D>> {
        let ring = Ring::must_new(self.points.clone());
        let hits = ring.all_intersections(input);

        if hits.is_empty() {
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

        if hits.is_empty() {
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

    /// Extracts all polygons from raw bytes representing a GeoJSON file, along with the string
    /// key/value properties. Only the first polygon from multipolygons is returned. If
    /// `require_in_bounds` is set, then the polygon must completely fit within the `gps_bounds`.
    pub fn from_geojson_bytes(
        raw_bytes: &[u8],
        gps_bounds: &GPSBounds,
        require_in_bounds: bool,
    ) -> Result<Vec<(Polygon, Tags)>> {
        let raw_string = std::str::from_utf8(raw_bytes)?;
        let geojson = raw_string.parse::<geojson::GeoJson>()?;
        let features = match geojson {
            geojson::GeoJson::Feature(feature) => vec![feature],
            geojson::GeoJson::FeatureCollection(collection) => collection.features,
            _ => anyhow::bail!("Unexpected geojson: {:?}", geojson),
        };

        let mut results = Vec::new();
        for feature in features {
            if let Some(geom) = &feature.geometry {
                let raw_pts = match &geom.value {
                    geojson::Value::Polygon(pts) => pts,
                    // If there are multiple, just use the first
                    geojson::Value::MultiPolygon(polygons) => &polygons[0],
                    _ => {
                        continue;
                    }
                };
                // TODO Handle holes
                let gps_pts: Vec<LonLat> = raw_pts[0]
                    .iter()
                    .map(|pt| LonLat::new(pt[0], pt[1]))
                    .collect();
                let pts = if !require_in_bounds {
                    gps_bounds.convert(&gps_pts)
                } else if let Some(pts) = gps_bounds.try_convert(&gps_pts) {
                    pts
                } else {
                    continue;
                };
                if let Ok(ring) = Ring::new(pts) {
                    let mut tags = Tags::empty();
                    for (key, value) in feature.properties_iter() {
                        if let Some(value) = value.as_str() {
                            tags.insert(key, value);
                        }
                    }
                    results.push((ring.into_polygon(), tags));
                }
            }
        }
        Ok(results)
    }

    pub fn simplify(&self, epsilon: f64) -> Polygon {
        self.to_geo().simplifyvw_preserve(&epsilon).into()
    }

    /// An arbitrary placeholder value, when Option types aren't worthwhile
    pub fn dummy() -> Self {
        Polygon::rectangle(0.1, 0.1)
    }

    // A less verbose way of invoking the From/Into impl. Note this hides a potentially expensive
    // clone. The eventual goal is for Polygon to directly wrap a geo::Polygon, at which point this
    // cost goes away.
    fn to_geo(&self) -> geo::Polygon {
        self.clone().into()
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

// Note that this could crash on invalid rings
impl From<geo::Polygon> for Polygon {
    fn from(poly: geo::Polygon) -> Self {
        let (exterior, interiors) = poly.into_inner();
        Polygon::with_holes(
            Ring::from(exterior),
            interiors.into_iter().map(Ring::from).collect(),
        )
    }
}

impl From<Polygon> for geo::Polygon {
    fn from(poly: Polygon) -> Self {
        if let Some(mut rings) = poly.rings {
            let exterior = rings.remove(0);
            let interiors: Vec<geo::LineString> =
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

fn from_multi(multi: geo::MultiPolygon) -> Vec<Polygon> {
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

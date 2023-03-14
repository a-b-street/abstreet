use std::collections::BTreeMap;
use std::fmt;

use anyhow::Result;
use geo::{
    Area, BooleanOps, Contains, ConvexHull, Intersects, MapCoordsInPlace, SimplifyVwPreserve,
};
use serde::{Deserialize, Serialize};

use crate::{
    Angle, Bounds, CornerRadii, Distance, GPSBounds, LonLat, PolyLine, Pt2D, Ring, Tessellation,
    Triangle,
};

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
pub struct Polygon {
    // [0] is the outer/exterior ring, and the others are holes/interiors
    pub(crate) rings: Vec<Ring>,
    // For performance reasons, some callers may want to generate the polygon's Rings and
    // Tessellation at the same time, instead of using earcutr.
    pub(crate) tessellation: Option<Tessellation>,
}

impl Polygon {
    pub fn with_holes(outer: Ring, mut inner: Vec<Ring>) -> Self {
        inner.insert(0, outer);
        Self {
            rings: inner,
            tessellation: None,
        }
    }

    pub fn from_rings(rings: Vec<Ring>) -> Self {
        assert!(!rings.is_empty());
        Self {
            rings,
            tessellation: None,
        }
    }

    pub(crate) fn pretessellated(rings: Vec<Ring>, tessellation: Tessellation) -> Self {
        Self {
            rings,
            tessellation: Some(tessellation),
        }
    }

    pub fn from_geojson(raw: &[Vec<Vec<f64>>]) -> Result<Self> {
        let mut rings = Vec::new();
        for pts in raw {
            let transformed: Vec<Pt2D> =
                pts.iter().map(|pair| Pt2D::new(pair[0], pair[1])).collect();
            rings.push(Ring::new(transformed)?);
        }
        Ok(Self::from_rings(rings))
    }

    pub fn from_triangle(tri: &Triangle) -> Self {
        Ring::must_new(vec![tri.pt1, tri.pt2, tri.pt3, tri.pt1]).into_polygon()
    }

    pub fn triangles(&self) -> Vec<Triangle> {
        Tessellation::from(self.clone()).triangles()
    }

    /// Does this polygon contain the point in its interior?
    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.to_geo().contains(&geo::Point::from(pt))
    }

    pub fn get_bounds(&self) -> Bounds {
        // Interiors should always be strictly contained within the polygon's exterior
        Bounds::from(self.get_outer_ring().points())
    }

    /// Transformations must preserve Rings.
    fn transform<F: Fn(&Pt2D) -> Pt2D>(&self, f: F) -> Result<Self> {
        let mut rings = Vec::new();
        for ring in &self.rings {
            rings.push(Ring::new(ring.points().iter().map(&f).collect())?);
        }
        Ok(Self {
            rings,
            tessellation: self.tessellation.clone().take().map(|mut t| {
                t.transform(f);
                t
            }),
        })
    }

    pub fn translate(&self, dx: f64, dy: f64) -> Self {
        self.transform(|pt| pt.offset(dx, dy))
            .expect("translate shouldn't collapse Rings")
    }

    /// When `factor` is small, this may collapse Rings and thus fail.
    pub fn scale(&self, factor: f64) -> Result<Self> {
        self.transform(|pt| Pt2D::new(pt.x() * factor, pt.y() * factor))
    }

    /// When `factor` is known to be over 1, then scaling can't fail.
    pub fn must_scale(&self, factor: f64) -> Self {
        if factor < 1.0 {
            panic!("must_scale({factor}) might collapse Rings. Use scale()");
        }
        self.transform(|pt| Pt2D::new(pt.x() * factor, pt.y() * factor))
            .expect("must_scale collapsed a Ring")
    }

    pub fn rotate(&self, angle: Angle) -> Self {
        self.rotate_around(angle, self.center())
    }

    pub fn rotate_around(&self, angle: Angle, pivot: Pt2D) -> Self {
        self.transform(|pt| {
            let origin_pt = Pt2D::new(pt.x() - pivot.x(), pt.y() - pivot.y());
            let (sin, cos) = angle.normalized_radians().sin_cos();
            Pt2D::new(
                pivot.x() + origin_pt.x() * cos - origin_pt.y() * sin,
                pivot.y() + origin_pt.y() * cos + origin_pt.x() * sin,
            )
        })
        .expect("rotate_around shouldn't collapse Rings")
    }

    pub fn centered_on(&self, center: Pt2D) -> Self {
        let bounds = self.get_bounds();
        let dx = center.x() - bounds.width() / 2.0;
        let dy = center.y() - bounds.height() / 2.0;
        self.translate(dx, dy)
    }

    pub fn get_outer_ring(&self) -> &Ring {
        &self.rings[0]
    }

    pub fn into_outer_ring(mut self) -> Ring {
        self.rings.remove(0)
    }

    /// Returns the arithmetic mean of the outer ring's points. The result could wind up inside a
    /// hole in the polygon. Consider using `polylabel` too.
    pub fn center(&self) -> Pt2D {
        let mut pts = self.get_outer_ring().clone().into_points();
        pts.pop();
        Pt2D::center(&pts)
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    pub fn maybe_rectangle(width: f64, height: f64) -> Result<Self> {
        Ring::new(vec![
            Pt2D::new(0.0, 0.0),
            Pt2D::new(width, 0.0),
            Pt2D::new(width, height),
            Pt2D::new(0.0, height),
            Pt2D::new(0.0, 0.0),
        ])
        .map(|ring| ring.into_polygon())
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    /// Note this will panic if `width` or `height` is 0.
    pub fn rectangle(width: f64, height: f64) -> Self {
        Self::maybe_rectangle(width, height).unwrap()
    }

    pub fn rectangle_centered(center: Pt2D, width: Distance, height: Distance) -> Self {
        Self::rectangle(width.inner_meters(), height.inner_meters()).translate(
            center.x() - width.inner_meters() / 2.0,
            center.y() - height.inner_meters() / 2.0,
        )
    }

    pub fn rectangle_two_corners(pt1: Pt2D, pt2: Pt2D) -> Option<Self> {
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
        Some(Self::rectangle(width, height).translate(x1, y1))
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    pub fn maybe_rounded_rectangle<R: Into<CornerRadii>>(w: f64, h: f64, r: R) -> Option<Self> {
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
    pub fn pill(w: f64, h: f64) -> Self {
        let r = w.min(h) / 2.0;
        Self::maybe_rounded_rectangle(w, h, r).unwrap()
    }

    /// Top-left at the origin. Doesn't take Distance, because this is usually pixels, actually.
    /// If it's not possible to apply the specified radius, fallback to a regular rectangle.
    pub fn rounded_rectangle<R: Into<CornerRadii>>(w: f64, h: f64, r: R) -> Self {
        Self::maybe_rounded_rectangle(w, h, r).unwrap_or_else(|| Self::rectangle(w, h))
    }

    /// Union all of the polygons into one geo::MultiPolygon
    pub fn union_all_into_multipolygon(mut list: Vec<Self>) -> geo::MultiPolygon {
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

    pub fn intersection(&self, other: &Self) -> Result<Vec<Self>> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            from_multi(self.to_geo().intersection(&other.to_geo()))
        })) {
            Ok(result) => result,
            Err(err) => {
                println!("BooleanOps crashed: {err:?}");
                bail!("BooleanOps crashed: {err:?}");
            }
        }
    }

    pub fn difference(&self, other: &Self) -> Result<Vec<Self>> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            from_multi(self.to_geo().difference(&other.to_geo()))
        })) {
            Ok(result) => result,
            Err(err) => {
                println!("BooleanOps crashed: {err:?}");
                bail!("BooleanOps crashed: {err:?}");
            }
        }
    }

    pub fn convex_hull(list: Vec<Self>) -> Result<Self> {
        let mp: geo::MultiPolygon = list.into_iter().map(|p| p.to_geo()).collect();
        mp.convex_hull().try_into()
    }

    pub fn concave_hull(points: Vec<Pt2D>, concavity: u32) -> Result<Self> {
        use geo::k_nearest_concave_hull::KNearestConcaveHull;
        let points: Vec<geo::Point> = points.iter().map(|p| geo::Point::from(*p)).collect();
        points.k_nearest_concave_hull(concavity).try_into()
    }

    /// Find the "pole of inaccessibility" -- the most distant internal point from the polygon
    /// outline
    pub fn polylabel(&self) -> Pt2D {
        let pt = polylabel::polylabel(&self.to_geo(), &1.0).unwrap();
        Pt2D::new(pt.x(), pt.y())
    }

    /// Do two polygons intersect at all?
    pub fn intersects(&self, other: &Self) -> bool {
        self.to_geo().intersects(&other.to_geo())
    }

    /// Does this polygon intersect a polyline?
    pub fn intersects_polyline(&self, pl: &PolyLine) -> bool {
        self.to_geo().intersects(&pl.to_geo())
    }

    pub(crate) fn get_rings(&self) -> &[Ring] {
        &self.rings
    }

    /// Creates the outline around the polygon (both the exterior and holes), with the thickness
    /// half straddling the polygon and half of it just outside.
    ///
    /// Returns a `Tessellation` that may union together the outline from the exterior and multiple
    /// holes. Callers that need a `Polygon` must call `to_outline` on the individual `Rings`.
    pub fn to_outline(&self, thickness: Distance) -> Tessellation {
        Tessellation::union_all(
            self.rings
                .iter()
                .map(|r| Tessellation::from(r.to_outline(thickness)))
                .collect(),
        )
    }

    /// Usually m^2, unless the polygon is in screen-space
    pub fn area(&self) -> f64 {
        // Don't use signed_area, since we may work with polygons that have different orientations
        self.to_geo().unsigned_area()
    }

    /// Doesn't handle multiple crossings in and out.
    pub fn clip_polyline(&self, input: &PolyLine) -> Option<Vec<Pt2D>> {
        let hits = self.get_outer_ring().all_intersections(input);

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
        let ring = self.get_outer_ring();
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

    /// Optionally map the world-space points back to GPS.
    pub fn to_geojson(&self, gps: Option<&GPSBounds>) -> geojson::Geometry {
        let mut geom: geo::Geometry = self.to_geo().into();
        if let Some(ref gps_bounds) = gps {
            geom.map_coords_in_place(|c| {
                let gps = Pt2D::new(c.x, c.y).to_gps(gps_bounds);
                (gps.x(), gps.y()).into()
            });
        }

        geojson::Geometry {
            bbox: None,
            value: geojson::Value::from(&geom),
            foreign_members: None,
        }
    }

    /// Extracts all polygons from raw bytes representing a GeoJSON file, along with the string
    /// key/value properties. Only the first polygon from multipolygons is returned. If
    /// `require_in_bounds` is set, then the polygon must completely fit within the `gps_bounds`.
    pub fn from_geojson_bytes(
        raw_bytes: &[u8],
        gps_bounds: &GPSBounds,
        require_in_bounds: bool,
    ) -> Result<Vec<(Self, BTreeMap<String, String>)>> {
        let raw_string = std::str::from_utf8(raw_bytes)?;
        let geojson = raw_string.parse::<geojson::GeoJson>()?;
        let features = match geojson {
            geojson::GeoJson::Feature(feature) => vec![feature],
            geojson::GeoJson::FeatureCollection(collection) => collection.features,
            _ => bail!("Unexpected geojson: {:?}", geojson),
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
                    let mut tags = BTreeMap::new();
                    for (key, value) in feature.properties_iter() {
                        if let Some(value) = value.as_str() {
                            tags.insert(key.to_string(), value.to_string());
                        }
                    }
                    results.push((ring.into_polygon(), tags));
                }
            }
        }
        Ok(results)
    }

    /// If simplification fails, just keep the original polygon
    pub fn simplify(&self, epsilon: f64) -> Self {
        self.to_geo()
            .simplify_vw_preserve(&epsilon)
            .try_into()
            .unwrap_or_else(|_| self.clone())
    }

    /// An arbitrary placeholder value, when Option types aren't worthwhile
    pub fn dummy() -> Self {
        Self::rectangle(0.1, 0.1)
    }

    // A less verbose way of invoking the From/Into impl. Note this hides a potentially expensive
    // clone.
    fn to_geo(&self) -> geo::Polygon {
        self.clone().into()
    }

    /// Convert to `geo` and also map from world-space to WGS84
    pub fn to_geo_wgs84(&self, gps_bounds: &GPSBounds) -> geo::Polygon<f64> {
        let mut p = self.to_geo();
        p.map_coords_in_place(|c| {
            let gps = Pt2D::new(c.x, c.y).to_gps(gps_bounds);
            (gps.x(), gps.y()).into()
        });
        p
    }

    pub fn from_geo_wgs84(mut polygon: geo::Polygon<f64>, gps_bounds: &GPSBounds) -> Result<Self> {
        polygon.map_coords_in_place(|c| {
            let pt = LonLat::new(c.x, c.y).to_pt(gps_bounds);
            (pt.x(), pt.y()).into()
        });
        polygon.try_into()
    }
}

impl fmt::Display for Polygon {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Polygon with {} rings", self.rings.len())?;
        for ring in &self.rings {
            writeln!(f, "  {}", ring)?;
        }
        Ok(())
    }
}

impl TryFrom<geo::Polygon> for Polygon {
    type Error = anyhow::Error;

    fn try_from(poly: geo::Polygon) -> Result<Self> {
        let (exterior, interiors) = poly.into_inner();
        let mut holes = Vec::new();
        for linestring in interiors {
            holes.push(Ring::try_from(linestring)?);
        }
        Ok(Polygon::with_holes(Ring::try_from(exterior)?, holes))
    }
}

impl From<Polygon> for geo::Polygon {
    fn from(mut poly: Polygon) -> Self {
        let exterior = poly.rings.remove(0);
        let interiors: Vec<geo::LineString> =
            poly.rings.into_iter().map(geo::LineString::from).collect();
        Self::new(exterior.into(), interiors)
    }
}

pub(crate) fn from_multi(multi: geo::MultiPolygon) -> Result<Vec<Polygon>> {
    let mut result = Vec::new();
    for polygon in multi {
        result.push(Polygon::try_from(polygon)?);
    }
    Ok(result)
}

use std::collections::HashSet;
use std::fmt;
use std::fmt::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{Distance, GPSBounds, Line, PolyLine, Polygon, Pt2D, Tessellation};

/// Maybe a misnomer, but like a PolyLine, but closed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    // first equals last
    pts: Vec<Pt2D>,
}

impl Ring {
    pub fn new(pts: Vec<Pt2D>) -> Result<Ring> {
        if pts.len() < 3 {
            bail!("Can't make a ring with < 3 points");
        }
        if pts[0] != *pts.last().unwrap() {
            bail!("Can't make a ring with mismatching first/last points");
        }

        if let Some(pair) = pts.windows(2).find(|pair| pair[0] == pair[1]) {
            bail!("Ring has duplicate adjacent points near {}", pair[0]);
        }

        let result = Ring { pts };

        let mut seen_pts = HashSet::new();
        for pt in result.pts.iter().skip(1) {
            if seen_pts.contains(&pt.to_hashable()) {
                bail!("Ring has repeat non-adjacent points near {}", pt);
            }
            seen_pts.insert(pt.to_hashable());
        }

        Ok(result)
    }

    /// Use with caution. Ignores duplicate points
    pub fn unsafe_deduping_new(mut pts: Vec<Pt2D>) -> Result<Ring> {
        pts.dedup();
        if pts.len() < 3 {
            bail!("Can't make a ring with < 3 points");
        }
        if pts[0] != *pts.last().unwrap() {
            bail!("Can't make a ring with mismatching first/last points");
        }

        if let Some(pair) = pts.windows(2).find(|pair| pair[0] == pair[1]) {
            bail!("Ring has duplicate adjacent points near {}", pair[0]);
        }

        let result = Ring { pts };

        let mut seen_pts = HashSet::new();
        for pt in result.pts.iter().skip(1) {
            if seen_pts.contains(&pt.to_hashable()) {
                // Just spam logs instead of crashing
                println!("Ring has repeat non-adjacent points near {}", pt);
            }
            seen_pts.insert(pt.to_hashable());
        }

        Ok(result)
    }

    pub fn must_new(pts: Vec<Pt2D>) -> Ring {
        Ring::new(pts).unwrap()
    }

    /// First dedupes adjacent points
    pub fn deduping_new(mut pts: Vec<Pt2D>) -> Result<Self> {
        pts.dedup();
        Self::new(pts)
    }

    pub fn as_polyline(&self) -> PolyLine {
        PolyLine::unchecked_new(self.pts.clone())
    }

    /// Draws the ring with some thickness, with half of it straddling the interor of the ring, and
    /// half on the outside.
    pub fn to_outline(&self, thickness: Distance) -> Tessellation {
        // TODO Has a weird corner. Use the polygon offset thing instead?
        self.as_polyline().thicken_tessellation(thickness)
    }

    pub fn into_polygon(self) -> Polygon {
        Polygon::with_holes(self, Vec::new())
    }

    pub fn points(&self) -> &Vec<Pt2D> {
        &self.pts
    }
    pub fn into_points(self) -> Vec<Pt2D> {
        self.pts
    }

    /// Be careful with the order of results. Hits on an earlier line segment of other show up
    /// first, but if the ring hits a line segment at multiple points, who knows. Dedupes.
    pub fn all_intersections(&self, other: &PolyLine) -> Vec<Pt2D> {
        let mut hits = Vec::new();
        let mut seen = HashSet::new();
        for l1 in other.lines() {
            for l2 in self
                .pts
                .windows(2)
                .map(|pair| Line::must_new(pair[0], pair[1]))
            {
                if let Some(pt) = l1.intersection(&l2) {
                    if !seen.contains(&pt.to_hashable()) {
                        hits.push(pt);
                        seen.insert(pt.to_hashable());
                    }
                }
            }
        }
        hits
    }

    pub(crate) fn get_both_slices_btwn(
        &self,
        pt1: Pt2D,
        pt2: Pt2D,
    ) -> Option<(PolyLine, PolyLine)> {
        assert!(pt1 != pt2);
        let pl = self.as_polyline();

        let mut dist1 = pl.dist_along_of_point(pt1)?.0;
        let mut dist2 = pl.dist_along_of_point(pt2)?.0;
        if dist1 > dist2 {
            std::mem::swap(&mut dist1, &mut dist2);
        }
        if dist1 == dist2 {
            return None;
        }

        // TODO If we reversed the points, we need to reverse these results! Argh
        let candidate1 = pl.maybe_exact_slice(dist1, dist2).ok()?;
        let candidate2 = pl
            .maybe_exact_slice(dist2, pl.length())
            .ok()?
            .must_extend(pl.maybe_exact_slice(Distance::ZERO, dist1).ok()?);
        Some((candidate1, candidate2))
    }

    /// Assuming both points are somewhere along the ring, return the points in between the two, by
    /// tracing along the ring in the longer or shorter direction (depending on `longer`). If both
    /// points are the same, returns `None`.  The result is oriented from `pt1` to `pt2`.
    pub fn get_slice_between(&self, pt1: Pt2D, pt2: Pt2D, longer: bool) -> Option<PolyLine> {
        if pt1 == pt2 {
            return None;
        }
        let (candidate1, candidate2) = self.get_both_slices_btwn(pt1, pt2)?;
        let slice = if longer == (candidate1.length() > candidate2.length()) {
            candidate1
        } else {
            candidate2
        };
        if slice.first_pt() == pt1 {
            Some(slice)
        } else {
            // TODO Do we want to be paranoid here? Or just do the fix in get_both_slices_btwn
            // directly?
            Some(slice.reversed())
        }
    }

    /// Assuming both points are somewhere along the ring, return the points in between the two, by
    /// tracing along the ring in the shorter direction. If both points are the same, returns
    /// `None`.  The result is oriented from `pt1` to `pt2`.
    pub fn get_shorter_slice_between(&self, pt1: Pt2D, pt2: Pt2D) -> Option<PolyLine> {
        self.get_slice_between(pt1, pt2, false)
    }

    // TODO Rmove this one, fix all callers
    pub fn get_shorter_slice_btwn(&self, pt1: Pt2D, pt2: Pt2D) -> Option<PolyLine> {
        let (candidate1, candidate2) = self.get_both_slices_btwn(pt1, pt2)?;
        if candidate1.length() < candidate2.length() {
            Some(candidate1)
        } else {
            Some(candidate2)
        }
    }

    /// Extract all PolyLines and Rings. Doesn't handle crazy double loops and stuff.
    pub fn split_points(pts: &[Pt2D]) -> Result<(Vec<PolyLine>, Vec<Ring>)> {
        let mut seen = HashSet::new();
        let mut intersections = HashSet::new();
        for pt in pts {
            let pt = pt.to_hashable();
            if seen.contains(&pt) {
                intersections.insert(pt);
            } else {
                seen.insert(pt);
            }
        }
        intersections.insert(pts[0].to_hashable());
        intersections.insert(pts.last().unwrap().to_hashable());

        let mut polylines = Vec::new();
        let mut rings = Vec::new();
        let mut current = Vec::new();
        for pt in pts.iter().cloned() {
            current.push(pt);
            if intersections.contains(&pt.to_hashable()) && current.len() > 1 {
                if current[0] == pt && current.len() >= 3 {
                    rings.push(Ring::new(current.drain(..).collect())?);
                } else {
                    polylines.push(PolyLine::new(current.drain(..).collect())?);
                }
                current.push(pt);
            }
        }
        Ok((polylines, rings))
    }

    pub fn contains_pt(&self, pt: Pt2D) -> bool {
        self.as_polyline().dist_along_of_point(pt).is_some()
    }

    /// Produces a GeoJSON polygon, optionally mapping the world-space points back to GPS.
    pub fn to_geojson(&self, gps: Option<&GPSBounds>) -> geojson::Geometry {
        let mut pts = Vec::new();
        if let Some(gps) = gps {
            for pt in gps.convert_back(&self.pts) {
                pts.push(vec![pt.x(), pt.y()]);
            }
        } else {
            for pt in &self.pts {
                pts.push(vec![pt.x(), pt.y()]);
            }
        }
        geojson::Geometry::new(geojson::Value::Polygon(vec![pts]))
    }

    /// Translates the ring by a fixed offset.
    pub fn translate(mut self, dx: f64, dy: f64) -> Ring {
        for pt in &mut self.pts {
            *pt = pt.offset(dx, dy);
        }
        self
    }

    /// Find the "pole of inaccessibility" -- the most distant internal point from the polygon
    /// outline
    pub fn polylabel(&self) -> Pt2D {
        // TODO Refactor to_geo?
        let polygon = geo::Polygon::new(
            geo::LineString::from(
                self.pts
                    .iter()
                    .map(|pt| geo::Point::new(pt.x(), pt.y()))
                    .collect::<Vec<_>>(),
            ),
            Vec::new(),
        );
        let pt = polylabel::polylabel(&polygon, &1.0).unwrap();
        Pt2D::new(pt.x(), pt.y())
    }

    /// Look for "bad" rings that double back on themselves. These're likely to cause many
    /// downstream problems. "Bad" means the order of points doesn't match the order when sorting
    /// by angle from the center.
    ///
    /// TODO I spot many false positives. Look for better definitions -- maybe self-intersecting
    /// polygons?
    pub fn doubles_back(&self) -> bool {
        // Skip the first=last point
        let mut orig = self.pts.clone();
        orig.pop();
        // Polylabel is better than center; sometimes the center is very close to an edge
        let center = self.polylabel();

        let mut sorted = orig.clone();
        sorted.sort_by_key(|pt| pt.angle_to(center).normalized_degrees() as i64);

        // Align things again
        while sorted[0] != orig[0] {
            sorted.rotate_right(1);
        }

        // Do they match up?
        orig != sorted
    }

    /// Print the coordinates of this ring as a `geo::LineString` for easy bug reports
    pub fn as_geo_linestring(&self) -> String {
        let mut output = String::new();
        writeln!(output, "let line_string = geo_types::line_string![").unwrap();
        for pt in &self.pts {
            writeln!(output, "  (x: {}, y: {}),", pt.x(), pt.y()).unwrap();
        }
        writeln!(output, "];").unwrap();
        output
    }
}

impl fmt::Display for Ring {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Ring::new(vec![")?;
        for pt in &self.pts {
            writeln!(f, "  Pt2D::new({}, {}),", pt.x(), pt.y())?;
        }
        write!(f, "])")
    }
}

impl From<Ring> for geo::LineString {
    fn from(ring: Ring) -> Self {
        let coords = ring
            .pts
            .into_iter()
            .map(geo::Coord::from)
            .collect::<Vec<_>>();
        Self(coords)
    }
}

impl TryFrom<geo::LineString> for Ring {
    type Error = anyhow::Error;

    fn try_from(line_string: geo::LineString) -> Result<Self, Self::Error> {
        let pts: Vec<Pt2D> = line_string.0.into_iter().map(Pt2D::from).collect();
        Self::deduping_new(pts)
    }
}

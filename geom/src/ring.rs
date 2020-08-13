use crate::{Distance, Line, PolyLine, Polygon, Pt2D};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

// Maybe a misnomer, but like a PolyLine, but closed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    // first equals last
    pts: Vec<Pt2D>,
}

impl Ring {
    pub fn new(pts: Vec<Pt2D>) -> Result<Ring, String> {
        if pts.len() < 3 {
            return Err(format!("Can't make a ring with < 3 points"));
        }
        if pts[0] != *pts.last().unwrap() {
            return Err(format!(
                "Can't make a ring with mismatching first/last points"
            ));
        }

        if pts.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(format!("Ring has ~dupe adjacent pts"));
        }

        let result = Ring { pts };

        let mut seen_pts = HashSet::new();
        for pt in result.pts.iter().skip(1) {
            seen_pts.insert(pt.to_hashable());
        }
        if seen_pts.len() != result.pts.len() - 1 {
            return Err(format!("Ring has repeat non-adjacent points"));
        }

        Ok(result)
    }
    pub fn must_new(pts: Vec<Pt2D>) -> Ring {
        Ring::new(pts).unwrap()
    }

    pub fn to_outline(&self, thickness: Distance) -> Polygon {
        // TODO Has a weird corner. Use the polygon offset thing instead?
        PolyLine::unchecked_new(self.pts.clone()).make_polygons(thickness)
    }

    pub fn to_polygon(self) -> Polygon {
        Polygon::with_holes(self, Vec::new())
    }

    pub fn points(&self) -> &Vec<Pt2D> {
        &self.pts
    }
    pub fn into_points(self) -> Vec<Pt2D> {
        self.pts
    }

    // Be careful with the order of results. Hits on an earlier line segment of other show up first,
    // but if the ring hits a line segment at multiple points, who knows. Dedupes.
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

    pub fn get_shorter_slice_btwn(&self, pt1: Pt2D, pt2: Pt2D) -> PolyLine {
        assert!(pt1 != pt2);
        let pl = PolyLine::unchecked_new(self.pts.clone());

        let mut dist1 = pl.dist_along_of_point(pt1).unwrap().0;
        let mut dist2 = pl.dist_along_of_point(pt2).unwrap().0;
        if dist1 > dist2 {
            std::mem::swap(&mut dist1, &mut dist2);
        }

        let candidate1 = pl.exact_slice(dist1, dist2);
        let candidate2 = pl
            .exact_slice(dist2, pl.length())
            .must_extend(pl.exact_slice(Distance::ZERO, dist1));
        if candidate1.length() < candidate2.length() {
            candidate1
        } else {
            candidate2
        }
    }

    // Extract all PolyLines and Rings. Doesn't handle crazy double loops and stuff.
    pub fn split_points(pts: &Vec<Pt2D>) -> Result<(Vec<PolyLine>, Vec<Ring>), String> {
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
                    rings.push(Ring::must_new(current.drain(..).collect()));
                } else {
                    polylines.push(PolyLine::new(current.drain(..).collect())?);
                }
                current.push(pt);
            }
        }
        Ok((polylines, rings))
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

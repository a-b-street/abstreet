use crate::{Distance, Line, PolyLine, Polygon, Pt2D};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

// Maybe a misnomer, but like a PolyLine, but closed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ring {
    // first equals last
    pts: Vec<Pt2D>,
}

impl Ring {
    pub fn new(pts: Vec<Pt2D>) -> Ring {
        assert!(pts.len() >= 3);
        assert_eq!(pts[0], *pts.last().unwrap());

        // This checks no lines are too small. Could take the other approach and automatically
        // squish down points here and make sure the final result is at least EPSILON_DIST.
        // But probably better for the callers to do this -- they have better understanding of what
        // needs to be squished down, why, and how.
        if pts.windows(2).any(|pair| pair[0] == pair[1]) {
            panic!("Ring has ~dupe adjacent pts: {:?}", pts);
        }

        let result = Ring { pts };

        let mut seen_pts = HashSet::new();
        for pt in result.pts.iter().skip(1) {
            seen_pts.insert(pt.to_hashable());
        }
        if seen_pts.len() != result.pts.len() - 1 {
            panic!("Ring has repeat points: {}", result);
        }

        result
    }

    pub fn maybe_new(pts: Vec<Pt2D>) -> Option<Ring> {
        assert!(pts.len() >= 3);
        assert_eq!(pts[0], *pts.last().unwrap());

        if pts.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }

        let result = Ring { pts };

        let mut seen_pts = HashSet::new();
        for pt in result.pts.iter().skip(1) {
            seen_pts.insert(pt.to_hashable());
        }
        if seen_pts.len() != result.pts.len() - 1 {
            return None;
        }

        Some(result)
    }

    pub fn make_polygons(&self, thickness: Distance) -> Polygon {
        // TODO Has a weird corner. Use the polygon offset thing instead?
        PolyLine::new_for_ring(self.pts.clone()).make_polygons(thickness)
    }

    // Searches other in order
    pub fn first_intersection(&self, other: &PolyLine) -> Option<Pt2D> {
        for l1 in other.lines() {
            for l2 in self.pts.windows(2).map(|pair| Line::new(pair[0], pair[1])) {
                if let Some(pt) = l1.intersection(&l2) {
                    return Some(pt);
                }
            }
        }
        None
    }

    pub fn get_shorter_slice_btwn(&self, pt1: Pt2D, pt2: Pt2D) -> PolyLine {
        assert!(pt1 != pt2);
        let pl = PolyLine::new_for_ring(self.pts.clone());

        let mut dist1 = pl.dist_along_of_point(pt1).unwrap().0;
        let mut dist2 = pl.dist_along_of_point(pt2).unwrap().0;
        if dist1 > dist2 {
            std::mem::swap(&mut dist1, &mut dist2);
        }

        let candidate1 = pl.exact_slice(dist1, dist2);
        let candidate2 = pl
            .exact_slice(dist2, pl.length())
            .extend(pl.exact_slice(Distance::ZERO, dist1));
        if candidate1.length() < candidate2.length() {
            candidate1
        } else {
            candidate2
        }
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

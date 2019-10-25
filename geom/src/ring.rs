use crate::{Distance, PolyLine, Polygon, Pt2D};
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

    pub fn make_polygons(&self, thickness: Distance) -> Polygon {
        // TODO Has a weird corner. Use the polygon offset thing instead? And move the
        // implementation here, ideally.
        PolyLine::make_polygons_for_boundary(self.pts.clone(), thickness)
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
